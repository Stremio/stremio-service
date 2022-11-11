mod config;

use config::{UPDATE_REPO_OWNER, UPDATE_REPO_NAME, UPDATE_FILE_NAME};
use stremio_service::shared::{get_current_exe_dir, get_version_string};

use std::{error::Error, io::Cursor, path::PathBuf, process::Command};
use log::{error, info};
use octocrab::models::repos::Asset;
use semver::{Version, VersionReq};

struct Update {
    version: Version,
    assets: Vec<Asset>
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let current_version = get_version_string();
    info!("Fetching updates for v{}", current_version);

    let latest_update = get_latest_update(&(">".to_owned() + &current_version)).await?;
    match latest_update {
        Some(update) => {
            info!("Found update v{}", update.version.to_string());

            let asset = update.assets.iter().find_map(|asset| {
                match asset.name.as_str() == UPDATE_FILE_NAME {
                    true => Some(asset),
                    false => None
                }
            });

            match asset {
                Some(asset) => {
                    info!("Downloading {}...", asset.name);
                    let archive = reqwest::get(asset.browser_download_url.clone())
                        .await?
                        .bytes()
                        .await?;

                    let current_exe_dir = get_current_exe_dir();

                    info!("Extracting archive to {:?}...", current_exe_dir);
                    let extracted = zip_extract::extract(Cursor::new(archive), &current_exe_dir, true);

                    match extracted {
                        Ok(_) => info!("Successfully extracted archive."),
                        Err(e) => error!("Failed to extract archive: {}", e)
                    }
                },
                None => error!("Could not find the specified asset in the release.")
            }
        },
        None => error!("Failed to get new updates."),
    }

    run_service();

    Ok(())
}

async fn get_latest_update(version: &str) -> Result<Option<Update>, Box<dyn Error>> {
    let response = octocrab::instance()
        .repos(UPDATE_REPO_OWNER, UPDATE_REPO_NAME)
        .releases()
        .list()
        .send()
        .await;

    match response {
        Ok(page) => {
            let current_version = VersionReq::parse(version)?;
            let update: Option<Update> = page.items.iter().find_map(|release| {
                let version = Version::parse(&release.tag_name.replace("v", ""))
                    .expect("Failed to parse release version tag");

                match current_version.matches(&version) {
                    true => Some(Update {
                        version,
                        assets: release.assets.clone()
                    }),
                    false => None
                }
            });
        
            return Ok(update)
        },
        Err(e) => error!("Failed to fetch releases from {UPDATE_REPO_OWNER}/{UPDATE_REPO_NAME}: {}", e)
    }
    
    Ok(None)
}

fn run_service() {
    let current_exe_dir = get_current_exe_dir();
    let updater_binary_path = current_exe_dir.join(PathBuf::from("service"));

    let mut command = Command::new(updater_binary_path);
    command.arg("--skip-updater");
        
    match command.spawn() {
        Ok(process) => {
            let process_pid = process.id();
            info!("Stremio Service started. (PID {:?})", process_pid);
        },
        Err(err) => error!("Stremio Service couldn't be started: {err}")
    }
}