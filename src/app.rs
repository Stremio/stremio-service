// Copyright (C) 2017-2026 Smart Code OOD 203358507

use anyhow::Context;
use fslock::LockFile;
use log::{error, info};
#[cfg(all(feature = "bundled", any(target_os = "linux", target_os = "macos")))]
use std::path::Path;
use std::path::PathBuf;
use tao::{
    event::Event,
    event_loop::{ControlFlow, EventLoop, EventLoopBuilder},
};
use tray_icon::{
    menu::{Menu, MenuEvent, MenuId, MenuItem},
    TrayIcon, TrayIconBuilder,
};

use crate::{
    config::Config,
    constants::{APP_ICON, STREMIO_URL},
    server::Server,
    updater::Updater,
    util::load_icon,
};
use urlencoding::encode;

/// Updater is supported only for non-linux operating systems.
#[cfg(not(target_os = "linux"))]
pub static IS_UPDATER_SUPPORTED: bool = true;
/// Updater is supported only for non-linux operating systems.
#[cfg(target_os = "linux")]
pub static IS_UPDATER_SUPPORTED: bool = false;

enum UserEvent {
    MenuEvent(MenuId),
}

pub struct Application {
    /// The video server process
    server: Server,
    config: Config,
}

impl Application {
    pub fn new(config: Config) -> Self {
        Self {
            server: Server::new(config.server.clone()),
            config,
        }
    }

    pub async fn run(&self) -> Result<(), anyhow::Error> {
        let mut lockfile = LockFile::open(&self.config.lockfile)?;

        if !lockfile.try_lock()? {
            info!("Exiting, another instance is running.");

            return Ok(());
        }

        #[cfg(all(feature = "bundled", any(target_os = "linux", target_os = "macos")))]
        make_it_autostart(self.config.home_dir.clone()).await;

        // NOTE: we do not need to run the Fruitbasket event loop but we do need to keep `app` in-scope for the full lifecycle of the app
        #[cfg(target_os = "macos")]
        let _fruit_app = register_apple_event_callbacks();

        // Showing the system tray icon as soon as possible to give the user a feedback
        let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
        let (mut system_tray, open_item_id, quit_item_id) =
            create_system_tray(&event_loop, &self.config.tray_icon)?;

        let current_version = env!("CARGO_PKG_VERSION")
            .parse()
            .expect("Should always be valid");
        let updater = Updater::new(current_version, &self.config);
        let updated = updater.prompt_and_update().await;

        if updated {
            // Exit current process as the updater has spawn the
            // new version in a separate process.
            // We haven't started the server.js in this instance yet
            // so it is safe to run the second service by the updater
            return Ok(());
        }

        self.server.start().context("Failed to start server.js")?;
        // cheap to clone and interior mutability
        let mut server = self.server.clone();

        event_loop.run(move |event, _event_loop, control_flow| {
            *control_flow = ControlFlow::Wait;

            match event {
                Event::UserEvent(event) => match event {
                    UserEvent::MenuEvent(menu_id) => {
                        if menu_id == open_item_id {
                            open_stremio_web(None);
                        }
                        if menu_id == quit_item_id {
                            *control_flow = ControlFlow::Exit;
                        }
                    }
                },
                Event::LoopDestroyed => {
                    system_tray.take();

                    if let Err(err) = server.stop() {
                        error!("{err}")
                    }
                }
                _ => (),
            }
        });
    }
}

fn create_system_tray(
    event_loop: &EventLoop<UserEvent>,
    icon_dir: &PathBuf,
) -> Result<(Option<TrayIcon>, MenuId, MenuId), anyhow::Error> {
    let open_item = MenuItem::new("Open Stremio Web", true, None);
    let quit_item = MenuItem::new("Quit", true, None);

    let version_label = format!("v{}", env!("CARGO_PKG_VERSION"));
    let version_item = MenuItem::new(version_label.as_str(), false, None);

    let menu = Menu::new();
    menu.append_items(&[&open_item, &quit_item, &version_item])
        .context("Failed to append menu items")?;

    let icon = load_icon(APP_ICON);

    let tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_icon(icon)
        .with_temp_dir_path(icon_dir)
        .build()
        .context("Failed to build tray icon")?;

    let proxy = event_loop.create_proxy();
    tray_icon::menu::MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        proxy.send_event(UserEvent::MenuEvent(event.id)).ok();
    }));

    Ok((
        Some(tray_icon),
        open_item.id().to_owned(),
        quit_item.id().to_owned(),
    ))
}

/// Handles `stremio://` urls by replacing the custom scheme with `https://`
/// and opening it.
/// Either opens the Addon installation link or the Web UI url
pub fn handle_stremio_protocol(open_url: String) {
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
        Err(e) => error!("Failed to open Stremio Web: {}", e),
    }
}

/// Only for Linux and MacOS
#[cfg(all(feature = "bundled", any(target_os = "linux", target_os = "macos")))]
async fn make_it_autostart(home_dir: impl AsRef<Path>) {
    #[cfg(target_os = "linux")]
    {
        use crate::{
            constants::{AUTOSTART_CONFIG_PATH, DESKTOP_FILE_NAME, DESKTOP_FILE_PATH},
            util::create_dir_if_does_not_exists,
        };
        use ashpd::desktop::background::Background;

        if Path::new("/.flatpak-info").exists() {
            let request = Background::request().auto_start(true);

            if let Err(e) = request.send().await.and_then(|r| r.response()) {
                error!("Failed to request autostart: {}", e);
            }
        } else {
            create_dir_if_does_not_exists(&home_dir.as_ref().join(AUTOSTART_CONFIG_PATH));

            let from = PathBuf::from(DESKTOP_FILE_PATH).join(DESKTOP_FILE_NAME);
            let to = home_dir
                .as_ref()
                .join(AUTOSTART_CONFIG_PATH)
                .join(DESKTOP_FILE_NAME);

            if !to.exists() {
                if let Err(e) = std::fs::copy(from, to) {
                    error!("Failed to copy desktop file to autostart location: {}", e);
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        use crate::{
            constants::{APP_IDENTIFIER, APP_NAME, LAUNCH_AGENTS_PATH},
            util::create_dir_if_does_not_exists,
        };

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

        let launch_agents_path = home_dir.as_ref().join(LAUNCH_AGENTS_PATH);
        create_dir_if_does_not_exists(&launch_agents_path);

        let plist_path = launch_agents_path.join(format!("{}.plist", APP_IDENTIFIER));
        if !plist_path.exists() {
            if let Err(e) = std::fs::write(plist_path, plist_launch_agent.as_bytes()) {
                error!("Failed to create a plist file in LaunchAgents dir: {}", e);
            }
        }
    }
}

#[cfg(target_os = "macos")]
fn register_apple_event_callbacks() -> fruitbasket::FruitApp<'static> {
    use fruitbasket::{FruitApp, FruitCallbackKey};

    let mut app = FruitApp::new();

    app.register_apple_event(fruitbasket::kInternetEventClass, fruitbasket::kAEGetURL);
    app.register_callback(
        FruitCallbackKey::Method("handleEvent:withReplyEvent:"),
        Box::new(move |event| {
            let open_url: String = fruitbasket::parse_url_event(event);
            handle_stremio_protocol(open_url);
        }),
    );

    app
}
