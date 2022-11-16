mod config;
mod server;

use std::{error::Error, process::Command, path::PathBuf};
use log::{error, info};
use clap::Parser;
use octocrab::models::repos::Asset;
use reqwest::Url;
use semver::{Version, VersionReq};
use tao::{event_loop::{EventLoop, ControlFlow}, menu::{ContextMenu, MenuItemAttributes, MenuId}, system_tray::{SystemTrayBuilder, SystemTray}, TrayId, event::Event};
use rust_embed::RustEmbed;

use config::{DATA_DIR, STREMIO_URL, UPDATE_REPO_OWNER, UPDATE_REPO_NAME, UPDATE_FILE_NAME, UPDATE_FILE_EXT};
use server::Server;
use stremio_service::shared::{load_icon, get_version_string, join_current_exe_dir};

#[derive(RustEmbed)]
#[folder = "icons"]
struct Icons;

#[derive(Parser, Debug)]
pub struct Options {
    #[clap(short, long)]
    pub skip_updater: bool,
}

struct Update {
    version: Version,
    file: Asset
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let options = Options::parse();

    let data_location = dirs::home_dir()
        .expect("Failed to get home dir")
        .join(DATA_DIR);

    std::fs::create_dir_all(data_location.clone())?;

    let lock_path = data_location.join("lock");
    if lock_path.exists() {
        info!("Exiting, another instance is running.");
        return Ok(())
    }

    std::fs::File::create(lock_path.clone())?;

    #[cfg(not(target_os = "linux"))]
    if !options.skip_updater {
        let current_version = get_version_string();
        info!("Fetching updates for v{}", current_version);

        match fetch_update(&current_version).await {
            Ok(response) => {
                match response {
                    Some(update) => {
                        info!("Found update v{}", update.version.to_string());
                        run_updater(update.file.browser_download_url);
                        remove_lock_file(lock_path);
                        return Ok(());
                    },
                    None => {}
                }
            },
            Err(e) => error!("Failed to fetch updates: {}", e)
        }
    }

    let mut server = Server::new(data_location);
    server.update().await?;
    server.start()?;

    let event_loop = EventLoop::new();

    let (mut system_tray, open_item_id, quit_item_id) = create_system_tray(&event_loop)?;

    event_loop.run(move |event, _event_loop, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::MenuEvent {
                menu_id,
                ..
            } => {
                if menu_id == open_item_id {
                    match open::that(STREMIO_URL) {
                        Ok(_) => info!("Opened Stremio Web in the browser"),
                        Err(e) => error!("Failed to open Stremio Web: {}", e)
                    }
                }
                if menu_id == quit_item_id {
                    system_tray.take();
                    *control_flow = ControlFlow::Exit;
                }
            },
            Event::LoopDestroyed => {
                server.stop();
                remove_lock_file(lock_path.clone());
            },
            _ => (),
        }
    });
}

fn remove_lock_file(path: PathBuf) {
    std::fs::remove_file(path.clone())
        .expect("Failed to delete lock file.");
}

fn create_system_tray(event_loop: &EventLoop<()>) -> Result<(Option<SystemTray>, MenuId, MenuId), Box<dyn Error>> {
    let mut tray_menu = ContextMenu::new();
    let open_item = tray_menu.add_item(MenuItemAttributes::new("Open Stremio Web"));
    let quit_item = tray_menu.add_item(MenuItemAttributes::new("Quit"));

    let version_item_label = format!("v{}", get_version_string());
    let version_item = MenuItemAttributes::new(version_item_label.as_str())
        .with_enabled(false);
    tray_menu.add_item(version_item);

    let icon_file = Icons::get("icon.png")
        .expect("Failed to get icon file");
    let icon = load_icon(icon_file.data.as_ref());

    let system_tray = SystemTrayBuilder::new(icon.clone(), Some(tray_menu))
        .with_id(TrayId::new("main"))
        .build(event_loop)
        .unwrap();

    Ok((
        Some(system_tray),
        open_item.id(),
        quit_item.id()
    ))
}

async fn fetch_update(version: &str) -> Result<Option<Update>, Box<dyn Error>> {
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

fn run_updater(update_url: Url) {
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