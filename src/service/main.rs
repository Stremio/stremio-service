mod updater;
mod server;

use std::{error::Error, path::PathBuf};
use fslock::LockFile;
use log::{error, info};
use clap::Parser;
use tao::{event_loop::{EventLoop, ControlFlow}, menu::{ContextMenu, MenuItemAttributes, MenuId}, system_tray::{SystemTrayBuilder, SystemTray}, TrayId, event::Event};
#[cfg(not(target_os = "linux"))]
use native_dialog::{MessageDialog, MessageType};
use rust_embed::RustEmbed;

#[cfg(not(target_os = "linux"))]
use updater::{fetch_update, run_updater};
use server::Server;
use stremio_service::{
    config::{DATA_DIR, STREMIO_URL, DESKTOP_FILE_PATH, DESKTOP_FILE_NAME, AUTOSTART_CONFIG_PATH},
    shared::load_icon
};

#[derive(RustEmbed)]
#[folder = "icons"]
struct Icons;

#[derive(Parser, Debug)]
pub struct Options {
    #[clap(short, long)]
    pub skip_updater: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    #[cfg(not(target_os = "linux"))]
    let options = Options::parse();

    let home_dir = dirs::home_dir()
        .expect("Failed to get home dir");
    let data_location = home_dir.join(DATA_DIR);

    std::fs::create_dir_all(data_location.clone())?;

    let lock_path = data_location.join("lock");
    let mut lockfile = LockFile::open(&lock_path)?;

    if !lockfile.try_lock()? {
        info!("Exiting, another instance is running.");
        return Ok(())
    }

    #[cfg(target_os = "linux")]
    make_it_autostart(home_dir);

    #[cfg(not(target_os = "linux"))]
    if !options.skip_updater {
        let current_version = env!("CARGO_PKG_VERSION");
        info!("Fetching updates for v{}", current_version);

        match fetch_update(&current_version).await {
            Ok(response) => {
                match response {
                    Some(update) => {
                        info!("Found update v{}", update.version.to_string());

                        let title = "Stremio Service";
                        let message = format!("Update v{} is available.\nDo you want to update now?", update.version.to_string());
                        let do_update = MessageDialog::new()
                            .set_type(MessageType::Info)
                            .set_title(title)
                            .set_text(&message)
                            .show_confirm()
                            .unwrap();

                        if do_update {
                            run_updater(update.file.browser_download_url);
                            return Ok(());
                        }
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
            },
            _ => (),
        }
    });
}

fn make_it_autostart(home_dir: PathBuf) {
    let from = PathBuf::from(DESKTOP_FILE_PATH).join(DESKTOP_FILE_NAME);
    let to = PathBuf::from(home_dir).join(AUTOSTART_CONFIG_PATH).join(DESKTOP_FILE_NAME);

    if !to.exists() {
        if let Err(e) = std::fs::copy(from, to) {
            error!("Failed to copy desktop file to autostart location: {}", e);
        }
    }
}

fn create_system_tray(event_loop: &EventLoop<()>) -> Result<(Option<SystemTray>, MenuId, MenuId), Box<dyn Error>> {
    let mut tray_menu = ContextMenu::new();
    let open_item = tray_menu.add_item(MenuItemAttributes::new("Open Stremio Web"));
    let quit_item = tray_menu.add_item(MenuItemAttributes::new("Quit"));

    let version_item_label = format!("v{}", env!("CARGO_PKG_VERSION"));
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