//! Google Drive Integration Module
//! Handles authentication and file uploads

use anyhow::{Context, Result};
use google_drive3::oauth2::{InstalledFlowAuthenticator, InstalledFlowReturnMethod};
use google_drive3::{api::File, DriveHub};
use hyper;
use hyper_rustls::HttpsConnector;
use hyper_rustls::HttpsConnectorBuilder;
use std::path::Path;

pub struct DriveManager {
    hub: DriveHub<HttpsConnector<hyper::client::HttpConnector>>,
}

impl DriveManager {
    /// Initialize DriveManager by performing authentication
    /// Requires `client_secret.json` in the working directory
    pub async fn new() -> Result<Self> {
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
            auth,
        );

        Ok(Self { hub })
    }

    /// Upload a file to Google Drive
    /// Returns the web_view_link of the uploaded file
    pub async fn upload_file(&self, file_path: &Path, folder_id: Option<&str>) -> Result<String> {
        let filename = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| anyhow::anyhow!("Invalid filename"))?;

        let mut file_metadata = File::default();
        file_metadata.name = Some(filename.to_string());

        if let Some(fid) = folder_id {
            file_metadata.parents = Some(vec![fid.to_string()]);
        }

        let mime_type = match file_path.extension().and_then(|e| e.to_str()) {
            Some("mp4") => "video/mp4",
            _ => "application/octet-stream",
        };

        // Read file content
        let file = std::fs::File::open(file_path)?;

        let (res, file) = self
            .hub
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
