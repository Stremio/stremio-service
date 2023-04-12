use anyhow::anyhow;
use log::{error, info};
use reqwest::Url;
use semver::{Version, VersionReq};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{io::Write, path::PathBuf, process::Command};
use tokio::io::AsyncWriteExt;

use crate::app::Config;

pub struct Update {
    /// The new version that we update to
    pub version: Version,
    pub file: PathBuf,
}

#[derive(Debug)]
pub struct Updater {
    pub current_version: Version,
    pub next_version: VersionReq,
    pub endpoint: String,
    pub force_update: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateResponse {
    version_desc: Url,
    version: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileItem {
    // name: String,
    pub url: Url,
    pub checksum: String,
    os: String,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Descriptor {
    version: String,
    // tag: String,
    // released: String,
    files: Vec<FileItem>,
}

impl Updater {
    pub fn new(current_version: Version, config: &Config) -> Self {
        Self {
            next_version: VersionReq::parse(&format!(">{current_version}"))
                .expect("Version is type-safe"),
            current_version,
            endpoint: config.updater_endpoint.clone(),
            force_update: config.force_update,
        }
    }

    /// Updates the service only for non-linux OS and returns whether an update was made.
    pub async fn prompt_and_update(&self) -> bool {
        #[cfg(not(target_os = "linux"))]
        {
            info!("Fetching updates for >v{}", self.current_version);

            match self.autoupdate().await {
                Ok(Some(update)) => {
                    info!("Found update v{}", update.version.to_string());

                    self.run_updater_setup(update.file);
                    return true;
                }
                Ok(None) => info!("No new updates found"),
                Err(e) => error!("Failed to fetch updates: {e}"),
            }
        }

        false
    }

    async fn check_for_update(&self, force: bool) -> Result<(FileItem, Version), anyhow::Error> {
        let update_response = reqwest::get(self.endpoint.clone())
            .await?
            .json::<UpdateResponse>()
            .await?;
        let update_descriptor = reqwest::get(update_response.version_desc)
            .await?
            .json::<Descriptor>()
            .await?;

        if update_response.version != update_descriptor.version {
            return Err(anyhow!("Missmatched update versions"));
        }
        let installer = update_descriptor
            .files
            .iter()
            .find(|file_item| file_item.os == std::env::consts::OS)
            .expect("No update for this OS");
        let version = Version::parse(update_descriptor.version.as_str())?;
        if !force && !self.next_version.matches(&version) {
            return Err(anyhow!(
                "No new releases found that match the requirement of `{}`",
                self.next_version
            ));
        }
        Ok((installer.clone(), version))
    }

    async fn download_and_verify_installer(
        &self,
        url: Url,
        expected_sha256: &str,
    ) -> Result<PathBuf, anyhow::Error> {
        let mut installer_response = reqwest::get(url.clone()).await?;
        let size = installer_response.content_length();
        let mut downloaded: u64 = 0;
        let mut sha256 = Sha256::new();
        let temp_dir = std::env::temp_dir();
        let file_name = std::path::Path::new(url.path())
            .file_name()
            .expect("Invalid file name")
            .to_str()
            .expect("The path is not valid UTF-8")
            .to_string();
        let dest = temp_dir.join(&file_name);

        println!("Downloading {} to {}", url, dest.display());

        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(dest.clone())
            .await?;
        while let Some(chunk) = installer_response.chunk().await? {
            sha256.update(&chunk);
            file.write_all(&chunk).await?;
            if let Some(size) = size {
                downloaded += chunk.len() as u64;
                print!("\rProgress: {}%", downloaded * 100 / size);
            } else {
                print!(".");
            }
            std::io::stdout().flush().ok();
        }
        println!();
        let actual_sha256 = format!("{:x}", sha256.finalize());
        if actual_sha256 != expected_sha256 {
            tokio::fs::remove_file(dest).await?;
            return Err(anyhow::anyhow!("Checksum verification failed"));
        }
        println!("Checksum verified.");
        Ok(dest)
    }

    /// Fetches the latest update from the update server.
    pub async fn autoupdate(&self) -> Result<Option<Update>, anyhow::Error> {
        let (installer, version) = self.check_for_update(self.force_update).await?;
        let dest = self
            .download_and_verify_installer(installer.url, &installer.checksum)
            .await?;
        let update = Some(Update {
            version,
            file: dest,
        });
        Ok(update)
    }

    pub fn run_updater_setup(&self, file_path: PathBuf) {
        // TODO: Handle the macOS dmg installer
        let mut command = Command::new(&file_path);
        command.arg("/VERYSILENT");

        match command.spawn() {
            Ok(process) => {
                let process_pid = process.id();
                info!("Updater started. (PID {:?})", process_pid);
            }
            Err(err) => error!("Updater couldn't be started: {err}"),
        }
    }
}
