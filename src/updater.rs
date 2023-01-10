use anyhow::{anyhow, Context};
use log::{error, info};
use octocrab::models::repos::Asset;
use reqwest::Url;
use semver::{Version, VersionReq};
use std::{path::PathBuf, process::Command};

use crate::config::{UPDATE_FILE_EXT, UPDATE_FILE_NAME, UPDATE_REPO_NAME, UPDATE_REPO_OWNER};

pub struct Update {
    /// The new version that we update to
    pub version: Version,
    pub file: Asset,
}

#[derive(Debug)]
pub struct Updater {
    pub current_version: Version,
    pub next_version: VersionReq,
    pub updater_bin: PathBuf,
}

impl Updater {
    pub fn new(current_version: Version, updater_bin: PathBuf) -> Self {
        Self {
            next_version: VersionReq::parse(&format!(">{current_version}"))
                .expect("Version is type-safe"),
            current_version,
            updater_bin,
        }
    }

    /// Updates the service only for non-linux OS and returns whether an update was made.
    pub async fn prompt_and_update(&self) -> bool {
        // #[cfg(not(target_os = "linux"))]
        {
            use native_dialog::{MessageDialog, MessageType};

            info!("Fetching updates for >v{}", self.current_version);

            match self.fetch_update().await {
                Ok(Some(update)) => {
                    info!("Found update v{}", update.version.to_string());

                    let title = "Stremio Service";
                    let message = format!(
                        "Update v{} is available.\nDo you want to update now?",
                        update.version
                    );

                    let confirm_update = MessageDialog::new()
                        .set_type(MessageType::Info)
                        .set_title(title)
                        .set_text(&message);

                    let do_update = confirm_update.show_confirm().unwrap();

                    if do_update {
                        self.run_updater_bin(update.file.browser_download_url);

                        return true;
                    }
                }
                Ok(None) => info!("No new updates found"),
                Err(e) => error!("Failed to fetch updates: {e}"),
            }
        }

        false
    }

    pub async fn fetch_update(&self) -> Result<Option<Update>, anyhow::Error> {
        let response_page = octocrab::instance()
            .repos(UPDATE_REPO_OWNER, UPDATE_REPO_NAME)
            .releases()
            .list()
            .send()
            .await
            .context(anyhow!(
                "Failed to fetch releases from {UPDATE_REPO_OWNER}/{UPDATE_REPO_NAME}"
            ))?;

        let update = response_page
            .items
            .iter()
            .filter_map(|release| {
                let release_version = release.tag_name.replace('v', "");

                match Version::parse(&release_version) {
                    Ok(version) => Some((release, version)),
                    Err(_) => {
                        error!(
                            "Failed to parse version tag for fetched release '{release_name}' #{release_id} - '{release_version}'",
                            release_name = release.name.as_deref().unwrap_or("(Empty)"),
                            release_id = release.id
                        );
                        // skip this release
                        None
                    }
                }
            })
            .find_map(|(release, version)| {
                // check if the requirement for newer version is met
                // e.g. `>0.2.0`
                if !self.next_version.matches(&version) {
                    info!(
                        "No new releases found that match the requirement of `{}`",
                        self.next_version
                    );

                    return None;
                }

                release.assets.iter().find_map(|asset| {
                    let update_file_name = format!(
                        "{UPDATE_FILE_NAME}-{os}.{UPDATE_FILE_EXT}",
                        os = std::env::consts::OS,
                    );

                    // if the asset doesn't have the expected filename
                    // for the OS, file extension and file name - skip it
                    if asset.name != update_file_name {
                        return None;
                    }

                    Some(Update {
                        version: version.clone(),
                        file: asset.clone(),
                    })
                })
            });

        Ok(update)
    }

    pub fn run_updater_bin(&self, update_url: Url) {
        let mut command = Command::new(&self.updater_bin);
        command.arg(format!("--url={}", update_url));

        match command.spawn() {
            Ok(process) => {
                let process_pid = process.id();
                info!("Updater started. (PID {:?})", process_pid);
            }
            Err(err) => error!("Updater couldn't be started: {err}"),
        }
    }
}
