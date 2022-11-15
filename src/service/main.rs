mod config;
mod server;

use std::{error::Error, process::Command};
use log::{error, info};
use clap::Parser;
use tao::{event_loop::{EventLoop, ControlFlow}, menu::{ContextMenu, MenuItemAttributes, MenuId}, system_tray::{SystemTrayBuilder, SystemTray}, TrayId, event::Event};
use rust_embed::RustEmbed;

use config::{DATA_DIR, STREMIO_URL};
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
        let updater_binary_path = join_current_exe_dir("updater");
    
        let mut command = Command::new(updater_binary_path);
        match command.spawn() {
            Ok(process) => {
                let process_pid = process.id();
                info!("Updater started. (PID {:?})", process_pid);
            },
            Err(err) => error!("Updater couldn't be started: {err}")
        }

        return Ok(())
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
                std::fs::remove_file(lock_path.clone())
                    .expect("Failed to delete lock file.");
            },
            _ => (),
        }
    });
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
