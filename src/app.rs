// Copyright (C) 2017-2024 Smart Code OOD 203358507

use anyhow::{anyhow, Context, Error};
use fslock::LockFile;
use log::{error, info};
use rand::Rng;
use rust_embed::RustEmbed;
#[cfg(all(feature = "bundled", any(target_os = "linux", target_os = "macos")))]
use std::path::Path;
use std::path::PathBuf;
use tao::{
    event::Event,
    event_loop::{ControlFlow, EventLoop},
    menu::{ContextMenu, MenuId, MenuItemAttributes},
    system_tray::{SystemTray, SystemTrayBuilder},
    TrayId,
};
use url::Url;

use crate::{
    args::Args,
    constants::{STREMIO_URL, UPDATE_ENDPOINT},
    server::Server,
    updater::Updater,
    util::load_icon,
};
use urlencoding::encode;

use crate::server;

/// Updater is supported only for non-linux operating systems.
#[cfg(not(target_os = "linux"))]
pub static IS_UPDATER_SUPPORTED: bool = true;
/// Updater is supported only for non-linux operating systems.
#[cfg(target_os = "linux")]
pub static IS_UPDATER_SUPPORTED: bool = false;

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
    #[cfg_attr(any(not(feature = "bundled"), target_os = "windows"), allow(dead_code))]
    home_dir: PathBuf,

    /// The lockfile that guards against running multiple instances of the service.
    lockfile: PathBuf,

    /// The server configuration
    server: server::Config,
    pub updater_endpoint: Url,
    pub skip_update: bool,
    pub force_update: bool,
}

impl Config {
    /// Try to create by validating the application configuration.
    ///
    /// It will initialize the server [`server::Config`] and if it fails it will return an error.
    ///
    /// If `self_update` is `true` and it is a supported platform for the updater (see [`IS_UPDATER_SUPPORTED`])
    /// it will check for the existence of the `updater` binary at the given location.
    pub fn new(
        args: Args,
        home_dir: PathBuf,
        cache_dir: PathBuf,
        service_bins_dir: PathBuf,
    ) -> Result<Self, Error> {
        let server =
            server::Config::new(service_bins_dir).context("Server configuration failed")?;

        let lockfile = cache_dir.join("lock");

        let updater_endpoint = if let Some(endpoint) = args.updater_endpoint {
            endpoint
        } else {
            let mut url = Url::parse(Self::get_random_updater_endpoint().as_str())?;
            if args.release_candidate {
                url.query_pairs_mut().append_pair("rc", "true");
            }
            url
        };

        Ok(Self {
            updater_endpoint,
            home_dir,
            lockfile,
            server,
            skip_update: args.skip_updater,
            force_update: args.force_update,
        })
    }
    fn get_random_updater_endpoint() -> String {
        let mut rng = rand::thread_rng();
        let index = rng.gen_range(0..UPDATE_ENDPOINT.len());
        UPDATE_ENDPOINT[index].to_string()
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
        let mut lockfile = LockFile::open(&self.config.lockfile)?;

        if !lockfile.try_lock()? {
            info!("Exiting, another instance is running.");

            return Ok(());
        }

        #[cfg(all(feature = "bundled", any(target_os = "linux", target_os = "macos")))]
        make_it_autostart(self.config.home_dir.clone());

        // NOTE: we do not need to run the Fruitbasket event loop but we do need to keep `app` in-scope for the full lifecycle of the app
        #[cfg(target_os = "macos")]
        let _fruit_app = register_apple_event_callbacks();

        // Showing the system tray icon as soon as possible to give the user a feedback
        let event_loop = EventLoop::new();
        let (mut system_tray, open_item_id, quit_item_id) = create_system_tray(&event_loop)?;

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

/// Only for Linux and MacOS
#[cfg(all(feature = "bundled", any(target_os = "linux", target_os = "macos")))]
fn make_it_autostart(home_dir: impl AsRef<Path>) {
    #[cfg(target_os = "linux")]
    {
        use crate::{
            constants::{AUTOSTART_CONFIG_PATH, DESKTOP_FILE_NAME, DESKTOP_FILE_PATH},
            util::create_dir_if_does_not_exists,
        };

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
