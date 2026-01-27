//! Security module for handling configuration encryption
//! Supports "None", "Simple", and "Password" modes.

use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use anyhow::{anyhow, Result};
use argon2::{
    password_hash::{PasswordHasher, SaltString},
    Argon2,
};
use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};

/// Available encryption modes for the configuration file
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
pub enum EncryptionMode {
    /// No encryption (Plain JSON)
    #[default]
    None,
    /// Simple encryption (Obfuscation with hardcoded key)
    Simple,
    /// Strong encryption (User provided password)
    Password,
}

/// Structure representing the secured configuration on disk
#[derive(Serialize, Deserialize, Debug)]
pub struct SecuredConfig {
    pub version: u32,
    pub mode: EncryptionMode,
    /// Base64 encoded salt (used for Password mode)
    pub salt: Option<String>,
    /// Base64 encoded nonce (used for AES-GCM)
    pub nonce: Option<String>,
    /// The actual configuration content
    /// If mode is None, this is the plain JSON string
    /// If mode is Simple/Password, this is the Base64 encoded ciphertext
    pub data: String,
}

/// Wrapper for decrypted configuration data
pub struct DecryptedConfig {
    pub content: String,
    pub mode: EncryptionMode,
}

// Hardcoded key for "Simple" mode options
// This offers NO real security against reverse engineering, just obfuscation.
const SIMPLE_KEY_BYTES: &[u8; 32] = b"yt-shortmaker-simple-secure-key!";

impl SecuredConfig {
    /// Create a new SecuredConfig from plain JSON content
    pub fn new(content: String, mode: EncryptionMode, password: Option<&str>) -> Result<Self> {
        match mode {
            EncryptionMode::None => Ok(Self {
                version: 1,
                mode,
                salt: None,
                nonce: None,
                data: content,
            }),
            EncryptionMode::Simple => {
                let key = Key::<Aes256Gcm>::from_slice(SIMPLE_KEY_BYTES);
                let (ciphertext, nonce_str) = encrypt_data(content.as_bytes(), key)?;
                Ok(Self {
                    version: 1,
                    mode,
                    salt: None,
                    nonce: Some(nonce_str),
                    data: ciphertext,
                })
            }
            EncryptionMode::Password => {
                let pass =
                    password.ok_or_else(|| anyhow!("Password required for Password mode"))?;
                let salt = SaltString::generate(&mut OsRng);
                let key_bytes = derive_key(pass.as_bytes(), &salt)?;
                let key = Key::<Aes256Gcm>::from_slice(&key_bytes);

                let (ciphertext, nonce_str) = encrypt_data(content.as_bytes(), key)?;

                Ok(Self {
                    version: 1,
                    mode,
                    salt: Some(salt.as_str().to_string()),
                    nonce: Some(nonce_str),
                    data: ciphertext,
                })
            }
        }
    }

    /// Attempt to decrypt the configuration
    /// Returns the plain JSON string
    pub fn decrypt(&self, password: Option<&str>) -> Result<DecryptedConfig> {
        let content = match self.mode {
            EncryptionMode::None => self.data.clone(),
            EncryptionMode::Simple => {
                let key = Key::<Aes256Gcm>::from_slice(SIMPLE_KEY_BYTES);
                decrypt_data(&self.data, &self.nonce, key)?
            }
            EncryptionMode::Password => {
                let pass = password.ok_or_else(|| anyhow!("Password required for decryption"))?;
                let salt_str = self
                    .salt
                    .as_ref()
                    .ok_or_else(|| anyhow!("Missing salt for password mode"))?;
                // Parse salt string back to SaltString
                let salt =
                    SaltString::from_b64(salt_str).map_err(|e| anyhow!("Invalid salt: {}", e))?;

                let key_bytes = derive_key(pass.as_bytes(), &salt)?;
                let key = Key::<Aes256Gcm>::from_slice(&key_bytes);

                decrypt_data(&self.data, &self.nonce, key)?
            }
        };

        Ok(DecryptedConfig {
            content,
            mode: self.mode,
        })
    }
}

// --- Helper Functions ---

fn encrypt_data(data: &[u8], key: &Key<Aes256Gcm>) -> Result<(String, String)> {
    let cipher = Aes256Gcm::new(key);
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng); // 96-bits; unique per message
    let ciphertext = cipher
        .encrypt(&nonce, data)
        .map_err(|e| anyhow!("Encryption failure: {}", e))?;

    Ok((
        general_purpose::STANDARD.encode(ciphertext),
        general_purpose::STANDARD.encode(nonce),
    ))
}

fn decrypt_data(
    encrypted_b64: &str,
    nonce_b64: &Option<String>,
    key: &Key<Aes256Gcm>,
) -> Result<String> {
    let nonce_str = nonce_b64.as_ref().ok_or_else(|| anyhow!("Missing nonce"))?;

    let nonce_bytes = general_purpose::STANDARD.decode(nonce_str)?;
    let ciphertext = general_purpose::STANDARD.decode(encrypted_b64)?;

    let nonce = Nonce::from_slice(&nonce_bytes);
    let cipher = Aes256Gcm::new(key);

    let plaintext = cipher
        .decrypt(nonce, ciphertext.as_ref())
        .map_err(|_| anyhow!("Decryption failed (Wrong password or corrupted data)"))?;

    String::from_utf8(plaintext).map_err(|e| anyhow!("Invalid UTF-8 in decrypted data: {}", e))
}

fn derive_key(password: &[u8], salt: &SaltString) -> Result<[u8; 32]> {
    let argon2 = Argon2::default();

    // Use hash_password which uses the trait
    let hash = argon2
        .hash_password(password, salt)
        .map_err(|e| anyhow!("Key derivation failed: {}", e))?;

    let output = hash.hash.ok_or_else(|| anyhow!("No hash output"))?;

    // Create a 32-byte key from the output
    let mut key = [0u8; 32];
    let src = output.as_bytes();

    if src.len() >= 32 {
        key.copy_from_slice(&src[0..32]);
    } else {
        return Err(anyhow!("Derived key too short"));
    }

    Ok(key)
}
