use std::{error::Error, process::Command};
use log::{info, error};
use octocrab::models::repos::Asset;
use reqwest::Url;
use semver::{Version, VersionReq};

use stremio_service::{config::{UPDATE_REPO_OWNER, UPDATE_REPO_NAME, UPDATE_FILE_NAME, UPDATE_FILE_EXT}, shared::join_current_exe_dir};

pub struct Update {
    pub version: Version,
    pub file: Asset
}

pub async fn fetch_update(version: &str) -> Result<Option<Update>, Box<dyn Error>> {
    let response = octocrab::instance()
        .repos(UPDATE_REPO_OWNER, UPDATE_REPO_NAME)
        .releases()
        .list()
        .send()
        .await;

    match response {
        Ok(page) => {
            let next_version = VersionReq::parse(&(">".to_owned() + version))?;
            let update: Option<Update> = page.items.iter().find_map(|release| {
                let version = Version::parse(&release.tag_name.replace("v", ""))
                    .expect("Failed to parse release version tag");

                match next_version.matches(&version) {
                    true => {
                        release.assets.iter().find_map(|asset| {
                            let update_file_name = format!("{}-{}.{}", UPDATE_FILE_NAME, std::env::consts::OS, UPDATE_FILE_EXT);
                            match asset.name == update_file_name {
                                true => Some(Update {
                                    version: version.clone(),
                                    file: asset.clone()
                                }),
                                false => None
                            }
                        })
                    },
                    false => None
                }
            });
        
            return Ok(update)
        },
        Err(e) => error!("Failed to fetch releases from {UPDATE_REPO_OWNER}/{UPDATE_REPO_NAME}: {}", e)
    }
    
    Ok(None)
}

pub fn run_updater(update_url: Url) {
    let updater_binary_path = join_current_exe_dir("updater");
    
    let mut command = Command::new(updater_binary_path);
    command.arg(format!("--url={}", update_url));

    match command.spawn() {
        Ok(process) => {
            let process_pid = process.id();
            info!("Updater started. (PID {:?})", process_pid);
        },
        Err(err) => error!("Updater couldn't be started: {err}")
    }
}