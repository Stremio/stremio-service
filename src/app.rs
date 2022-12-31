use anyhow::{anyhow, Context};
use fslock::LockFile;
use log::{error, info};
use rust_embed::RustEmbed;
use std::path::{Path, PathBuf};
use tao::{
    event::Event,
    event_loop::{ControlFlow, EventLoop},
    menu::{ContextMenu, MenuId, MenuItemAttributes},
    system_tray::{SystemTray, SystemTrayBuilder},
    TrayId,
};

use crate::{
    config::{DATA_DIR, STREMIO_URL},
    server::Server,
    util::load_icon,
};
use urlencoding::encode;

use crate::server;

#[derive(RustEmbed)]
#[folder = "icons"]
struct Icons;

pub struct Application {
    /// The video server process
    server: Server,
    config: Config,
}

#[derive(Debug, Clone)]
pub struct Config {
    /// The Home directory of the user running the service
    /// used to make the application an autostart one (on `*nix` systems)
    home_dir: PathBuf,

    /// The data directory where the service will store data
    data_dir: PathBuf,

    /// The lockfile that guards against running multiple instances of the service.
    lockfile: PathBuf,

    /// The server.js configuration
    server: server::Config,

    /// Whether or not to run updating process
    ///
    /// Applicable only to non-linux OSes
    run_updater: bool,
}

impl Config {
    pub fn new(
        home_dir: PathBuf,
        server: server::Config,
        run_updater: impl Into<Option<bool>>,
    ) -> Self {
        let data_dir = home_dir.join(DATA_DIR);
        let lockfile = data_dir.join("lock");

        Self {
            home_dir,
            data_dir,
            lockfile,
            server,
            run_updater: run_updater.into().unwrap_or(true),
        }
    }
}

impl Application {
    pub fn new(config: Config) -> Self {
        Self {
            server: Server::new(config.server.clone()),
            config,
        }
    }

    pub async fn run(&self) -> Result<(), anyhow::Error> {
        std::fs::create_dir_all(&self.config.data_dir)
            .context("Failed to create the service data directory")?;

        let mut lockfile = LockFile::open(&self.config.lockfile)?;

        if !lockfile.try_lock()? {
            info!("Exiting, another instance is running.");

            return Ok(());
        }

        make_it_autostart(self.config.home_dir.clone());

        // NOTE: we do not need to run the Fruitbasket event loop but we do need to keep `app` in-scope for the full lifecycle of the app
        #[cfg(target_os = "macos")]
        let _fruit_app = register_apple_event_callbacks();

        if self.config.run_updater {
            check_for_updates().await
        }

        self.server.start().context("Failed to start server.js")?;
        // cheap to clone and interior mutability
        let mut server = self.server.clone();

        let event_loop = EventLoop::new();

        let (mut system_tray, open_item_id, quit_item_id) = create_system_tray(&event_loop)?;

        event_loop.run(move |event, _event_loop, control_flow| {
            *control_flow = ControlFlow::Wait;

            match event {
                Event::MenuEvent { menu_id, .. } => {
                    if menu_id == open_item_id {
                        open_stremio_web(None);
                    }
                    if menu_id == quit_item_id {
                        system_tray.take();
                        *control_flow = ControlFlow::Exit;
                    }
                }
                Event::LoopDestroyed => {
                    if let Err(err) = server.stop() {
                        error!("{err}")
                    }
                }
                _ => (),
            }
        });
    }
}

/// Updates the service only for non-linux OS
async fn check_for_updates() {
    #[cfg(not(target_os = "linux"))]
    {
        use crate::updater::{fetch_update, run_updater};
        use native_dialog::{MessageDialog, MessageType};

        let current_version = env!("CARGO_PKG_VERSION");
        info!("Fetching updates for v{}", current_version);

        match fetch_update(&current_version).await {
            Ok(response) => match response {
                Some(update) => {
                    info!("Found update v{}", update.version.to_string());

                    let title = "Stremio Service";
                    let message = format!(
                        "Update v{} is available.\nDo you want to update now?",
                        update.version.to_string()
                    );
                    let do_update = MessageDialog::new()
                        .set_type(MessageType::Info)
                        .set_title(title)
                        .set_text(&message)
                        .show_confirm()
                        .unwrap();

                    if do_update {
                        run_updater(update.file.browser_download_url)
                    }
                }
                None => {}
            },
            Err(e) => error!("Failed to fetch updates: {}", e),
        }
    }
}

/// Only for Linux and MacOS
#[cfg_attr(
    any(target_os = "linux", target_os = "macos"),
    allow(unused_variables)
)]
fn make_it_autostart(home_dir: impl AsRef<Path>) {
    #[cfg(target_os = "linux")]
    {
        use crate::{
            config::{AUTOSTART_CONFIG_PATH, DESKTOP_FILE_NAME, DESKTOP_FILE_PATH},
            util::create_dir_if_does_not_exists,
        };

        create_dir_if_does_not_exists(AUTOSTART_CONFIG_PATH);

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

    #[cfg(target_os = "macos")]
    {
        use crate::{
            config::{APP_IDENTIFIER, APP_NAME, LAUNCH_AGENTS_PATH},
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
        create_dir_if_does_not_exists(
            launch_agents_path
                .to_str()
                .expect("Failed to convert PathBuf to str"),
        );

        let plist_path = launch_agents_path.join(format!("{}.plist", APP_IDENTIFIER));
        if !plist_path.exists() {
            if let Err(e) = std::fs::write(plist_path, plist_launch_agent.as_bytes()) {
                error!("Failed to create a plist file in LaunchAgents dir: {}", e);
            }
        }
    }
}

fn create_system_tray(
    event_loop: &EventLoop<()>,
) -> Result<(Option<SystemTray>, MenuId, MenuId), anyhow::Error> {
    let mut tray_menu = ContextMenu::new();
    let open_item = tray_menu.add_item(MenuItemAttributes::new("Open Stremio Web"));
    let quit_item = tray_menu.add_item(MenuItemAttributes::new("Quit"));

    let version_item_label = format!("v{}", env!("CARGO_PKG_VERSION"));
    let version_item = MenuItemAttributes::new(version_item_label.as_str()).with_enabled(false);
    tray_menu.add_item(version_item);

    let icon_file = Icons::get("icon.png").ok_or_else(|| anyhow!("Failed to get icon file"))?;
    let icon = load_icon(icon_file.data.as_ref());

    let system_tray = SystemTrayBuilder::new(icon, Some(tray_menu))
        .with_id(TrayId::new("main"))
        .build(event_loop)
        .context("Failed to build the application system tray")?;

    Ok((Some(system_tray), open_item.id(), quit_item.id()))
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
