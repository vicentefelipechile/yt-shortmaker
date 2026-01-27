use anyhow::{Context, Result};
use google_drive3::oauth2::{InstalledFlowAuthenticator, InstalledFlowReturnMethod};
use google_drive3::{api::File, DriveHub};
use hyper_rustls::HttpsConnector;
use hyper_rustls::HttpsConnectorBuilder;
use std::path::Path;

pub struct DriveManager {
    hub: Option<DriveHub<HttpsConnector<hyper::client::HttpConnector>>>,
}

impl DriveManager {
    /// Initialize DriveManager with optional existing token data
    pub async fn new(initial_token_data: Option<String>) -> Result<Self> {
        // Load initial tokens if provided
        if let Some(data) = initial_token_data {
            if let Ok(_tokens) = serde_json::from_str::<serde_json::Value>(&data) {
                // We don't need to manually deserialize into MemoryStorage anymore
                // We rely on import_tokens called by consumer BEFORE new() or just ignored here
                // if we strictly follow standard flow.
                // However, preserving the signature is fine.
            }
        }

        Ok(Self { hub: None })
    }

    /// Perform authentication
    pub async fn authenticate(&mut self) -> Result<()> {
        self.authenticate_with_disk().await
    }

    /// Helper to sync from config to disk
    pub fn import_tokens(token_data: Option<&str>) -> Result<()> {
        if let Some(data) = token_data {
            std::fs::write("token_cache.json", data)?;
        }
        Ok(())
    }

    /// Helper to sync from disk to config
    pub fn export_tokens() -> Result<Option<String>> {
        if Path::new("token_cache.json").exists() {
            let data = std::fs::read_to_string("token_cache.json")?;
            // Verify it's valid JSON?
            if serde_json::from_str::<serde_json::Value>(&data).is_ok() {
                return Ok(Some(data));
            }
        }
        Ok(None)
    }

    // We will stick to the struct existing but with valid Authenticator

    pub async fn authenticate_with_disk(&mut self) -> Result<()> {
        let secret_path = "client_secret.json";
        let secret = google_drive3::oauth2::read_application_secret(secret_path)
            .await
            .context("Failed to read client_secret.json")?;

        let auth =
            InstalledFlowAuthenticator::builder(secret, InstalledFlowReturnMethod::HTTPRedirect)
                .persist_tokens_to_disk("token_cache.json")
                .build()
                .await
                .context("Failed to create authenticator")?;

        let hub = DriveHub::new(
            hyper::Client::builder().build(
                HttpsConnectorBuilder::new()
                    .with_native_roots()
                    .https_or_http()
                    .enable_http1()
                    .build(),
            ),
            auth.clone(),
        );

        self.hub = Some(hub);
        // self.auth = Some(auth); // Types are hard to match sometimes, simpler to just keep hub

        Ok(())
    }

    /// Upload a file to Google Drive
    /// Returns the web_view_link of the uploaded file
    pub async fn upload_file(&self, file_path: &Path, folder_id: Option<&str>) -> Result<String> {
        let hub = self
            .hub
            .as_ref()
            .context("Drive Manager not authenticated")?;

        let filename = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| anyhow::anyhow!("Invalid filename"))?;

        let file_metadata = File {
            name: Some(filename.to_string()),
            parents: folder_id.map(|fid| vec![fid.to_string()]),
            ..Default::default()
        };

        let mime_type = match file_path.extension().and_then(|e| e.to_str()) {
            Some("mp4") => "video/mp4",
            _ => "application/octet-stream",
        };

        // Read file content
        let file = std::fs::File::open(file_path)?;

        let (res, file) = hub
            .files()
            .create(file_metadata)
            .upload(file, mime_type.parse().unwrap())
            .await
            .map_err(|e| anyhow::anyhow!("Drive API Upload Error: {}", e))?;

        if res.status().is_success() {
            Ok(file
                .web_view_link
                .ok_or_else(|| anyhow::anyhow!("No web view link returned"))?)
        } else {
            Err(anyhow::anyhow!(
                "Upload request failed with status: {}",
                res.status()
            ))
        }
    }
}
