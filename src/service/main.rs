#![windows_subsystem = "windows"]
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
    config::{DATA_DIR, STREMIO_URL, DESKTOP_FILE_PATH, DESKTOP_FILE_NAME, AUTOSTART_CONFIG_PATH, LAUNCH_AGENTS_PATH, APP_IDENTIFIER, APP_NAME},
    shared::{load_icon, create_dir_if_does_not_exists}
};
use urlencoding::encode;
use fruitbasket::{FruitApp, FruitCallbackKey};

#[derive(RustEmbed)]
#[folder = "icons"]
struct Icons;

#[derive(Parser, Debug)]
pub struct Options {
    #[clap(short, long)]
    pub skip_updater: bool,
    #[clap(short, long)]
    pub open: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let options = Options::parse();

    if let Some(open_url) = options.open {
        handle_stremio_protocol(open_url);
    }

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

    make_it_autostart(home_dir);

    // NOTE: we do not need to run the Fruitbasket event loop but we do need to keep `app` in-scope for the full lifecycle of the app
    #[cfg(target_os = "macos")]
    let mut app = FruitApp::new();
    #[cfg(target_os = "macos")] {
        app.register_apple_event(fruitbasket::kInternetEventClass, fruitbasket::kAEGetURL);
        app.register_callback(
            FruitCallbackKey::Method("handleEvent:withReplyEvent:"),
            Box::new(move |event| {
                let open_url: String = fruitbasket::parse_url_event(event);
                handle_stremio_protocol(open_url);
            }),
        );
    }

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

    let mut server = Server::new();
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
                    open_stremio_web(None);
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
    #[cfg(target_os = "linux")] {
        create_dir_if_does_not_exists(AUTOSTART_CONFIG_PATH);

        let from = PathBuf::from(DESKTOP_FILE_PATH).join(DESKTOP_FILE_NAME);
        let to = PathBuf::from(home_dir).join(AUTOSTART_CONFIG_PATH).join(DESKTOP_FILE_NAME);

        if !to.exists() {
            if let Err(e) = std::fs::copy(from, to) {
                error!("Failed to copy desktop file to autostart location: {}", e);
            }
        }
    }

    #[cfg(target_os = "macos")] {
        let plist_launch_agent = format!("
            <?xml version=\"1.0\" encoding=\"UTF-8\"?>
            <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">
            <plist version=\"1.0\">
            <dict>  
                <key>Label</key>
                <string>{}</string>
                <key>ProgramArguments</key>
                <array>
                    <string>/usr/bin/open</string>
                    <string>-a</string>
                    <string>{}</string>
                </array>
                <key>RunAtLoad</key>
                <true/>
            </dict>
            </plist>
        ", APP_IDENTIFIER, APP_NAME);

        let launch_agents_path = PathBuf::from(LAUNCH_AGENTS_PATH);
        create_dir_if_does_not_exists(
            launch_agents_path.to_str()
                .expect("Failed to convert PathBuf to str")
        );

        let plist_path = launch_agents_path.join(format!("{}.plist", APP_IDENTIFIER));
        if !plist_path.exists() {
            if let Err(e) = std::fs::write(plist_path, plist_launch_agent.as_bytes()) {
                error!("Failed to create a plist file in LaunchAgents dir: {}", e);
            }
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

fn handle_stremio_protocol(open_url: String) {
    if open_url.starts_with("stremio://") {
        let url = open_url.replace("stremio://", "https://");
        open_stremio_web(Some(url));
    }
}

fn open_stremio_web(addon_manifest_url: Option<String>) {
    let mut url = STREMIO_URL.to_string();
    if let Some(p) = addon_manifest_url {
        url = format!("{}/#/addons?addon={}", STREMIO_URL, &encode(&p));
    }

    match open::that(url) {
        Ok(_) => info!("Opened Stremio Web in the browser"),
        Err(e) => error!("Failed to open Stremio Web: {}", e)
    }
}