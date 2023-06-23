// Copyright (C) 2017-2023 Smart code 203358507

#[cfg(all(feature = "bundled", any(target_os = "linux", target_os = "macos")))]
use std::path::Path;
use std::{
    fmt::{Debug, Display},
    path::PathBuf,
    str::FromStr,
    time::Duration,
};

use anyhow::{bail, Context, Error};
use fslock::LockFile;
use futures_util::FutureExt;
use log::{error, info};
use rand::Rng;
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};
use tao::{
    event::Event,
    event_loop::{ControlFlow, EventLoop, EventLoopProxy},
};
use tokio::time::{interval_at, sleep, Instant};
use tokio_stream::{wrappers::IntervalStream, StreamExt};
use url::Url;
use urlencoding::encode;

use crate::{
    args::Args,
    config::{DATA_DIR, STREMIO_URL, UPDATE_ENDPOINT},
    server::{Info, Server},
    updater::Updater,
};

use crate::server;

use self::tray_menu::{
    MenuEvent, TrayMenu, OPEN_MENU, QUIT_MENU, RESTART_SERVER_MENU, START_SERVER_MENU,
    STOP_SERVER_MENU,
};

pub mod tray_menu;

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

#[derive(Debug, Default, Clone)]
pub struct TrayStatus {
    server_js: ServerTrayStatus,
}

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
enum ServerTrayStatus {
    #[default]
    Stopped,
    Running {
        #[serde(flatten)]
        info: Info,
    },
}

#[derive(Debug, Clone)]
pub struct Config {
    /// The Home directory of the user running the service
    /// used to make the application an autostart one (on `*nix` systems)
    #[cfg_attr(any(not(feature = "bundled"), target_os = "windows"), allow(dead_code))]
    home_dir: PathBuf,

    /// The data directory where the service will store data
    data_dir: PathBuf,

    /// The lockfile that guards against running multiple instances of the service.
    lockfile: PathBuf,

    /// The server.js configuration
    server: server::Config,
    pub updater_endpoint: Url,
    pub skip_update: bool,
    pub force_update: bool,
}

impl Config {
    /// Try to create by validating the application configuration.
    ///
    /// It will initialize the server.js [`server::Config`] and if it fails it will return an error.
    ///
    /// If `self_update` is `true` and it is a supported platform for the updater (see [`IS_UPDATER_SUPPORTED`])
    /// it will check for the existence of the `updater` binary at the given location.
    pub fn new(args: Args, home_dir: PathBuf, service_bins_dir: PathBuf) -> Result<Self, Error> {
        let server =
            server::Config::at_dir(service_bins_dir).context("Server.js configuration failed")?;

        let data_dir = home_dir.join(DATA_DIR);
        let lockfile = data_dir.join("lock");

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
            data_dir,
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
    pub const SERVER_STATUS_EVERY: Duration = Duration::from_secs(30);

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

        #[cfg(all(feature = "bundled", any(target_os = "linux", target_os = "macos")))]
        make_it_autostart(self.config.home_dir.clone());

        // NOTE: we do not need to run the Fruitbasket event loop but we do need to keep `app` in-scope for the full lifecycle of the app
        #[cfg(target_os = "macos")]
        let _fruit_app = register_apple_event_callbacks();

        // Showing the system tray icon as soon as possible to give the user a feedback
        let event_loop: EventLoop<MenuEvent> = EventLoop::with_user_event();

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

        let server_info = self
            .server
            .start()
            .await
            .context("Failed to start Server")?;

        self.server.run_logger();

        let tray_status = TrayStatus {
            server_js: ServerTrayStatus::Running {
                info: Info {
                    config: server_info.config.clone(),
                    version: server_info.version,
                    server_url: server_info.server_url,
                },
            },
        };

        let mut tray_menu = TrayMenu::new(&event_loop)?;
        tray_menu.set_status(tray_status);

        let stats_updater =
            Self::run_tray_status_updater(self.server.clone(), event_loop.create_proxy());
        tokio::spawn(stats_updater);

        // cheap to clone and interior mutability
        let server = self.server.clone();

        event_loop.run(move |event, _event_loop, control_flow| {
            *control_flow = ControlFlow::Wait;

            match event {
                Event::MenuEvent { menu_id, .. } if menu_id == *OPEN_MENU => {
                    // FIXME: call with the app's server_url from the command!
                    StremioWeb::OpenWeb { server_url: None }.open()
                }
                Event::MenuEvent { menu_id, .. } if menu_id == *QUIT_MENU => {
                    // drop(tray_menu);
                    *control_flow = ControlFlow::Exit;
                }
                Event::LoopDestroyed => {
                    if let Err(err) = futures::executor::block_on(server.stop()) {
                        error!("{err}")
                    }
                }
                Event::MenuEvent { menu_id, .. } if menu_id == *START_SERVER_MENU => {
                    if let Err(err) = futures::executor::block_on(server.start()) {
                        error!("Starting: {err}")
                    } else {
                        info!("Server has been started")
                    }
                }
                Event::MenuEvent { menu_id, .. } if menu_id == *STOP_SERVER_MENU => {
                    if let Err(err) = futures::executor::block_on(server.stop()) {
                        error!("{err}")
                    } else {
                        info!("Server has been shut down")
                    }
                }
                Event::MenuEvent { menu_id, .. } if menu_id == *RESTART_SERVER_MENU => {
                    futures::executor::block_on(server.restart().map(drop))
                }
                Event::UserEvent(menu_event) => match menu_event {
                    MenuEvent::UpdateTray(new_tray) => tray_menu.set_status(new_tray),
                },
                _ => (),
            }
        });
    }

    // async fn run_tray_status_updater(&self, tray_menu: Arc<Mutex<TrayMenu>>) {
    async fn run_tray_status_updater(server: Server, event_loop_proxy: EventLoopProxy<MenuEvent>) {
        let mut interval = IntervalStream::new(interval_at(
            Instant::now() + Self::SERVER_STATUS_EVERY,
            Self::SERVER_STATUS_EVERY,
        ));

        while let Some(_instant) = interval.next().await {
            let info = server.update_status().await;

            let status = match info {
                Some(info) => TrayStatus {
                    server_js: ServerTrayStatus::Running { info: info.clone() },
                },
                None => TrayStatus {
                    server_js: ServerTrayStatus::Stopped,
                },
            };

            info!("Server status updated: {status:#?}");

            match event_loop_proxy.send_event(MenuEvent::UpdateTray(status)) {
                Ok(_) => {
                    // do nothing
                }
                Err(err) => error!("Failed to send new status for tray menu. {err}"),
            }
        }
    }
}

/// Addon's `stremio://` prefixed url
pub struct AddonUrl {
    url: Url,
}
impl FromStr for AddonUrl {
    type Err = anyhow::Error;

    fn from_str(open_url: &str) -> Result<Self, Self::Err> {
        if open_url.starts_with("stremio://") {
            let url = open_url.replace("stremio://", "https://").parse::<Url>()?;

            return Ok(Self { url });
        }

        bail!("Stremio's addon protocol url starts with stremio://")
    }
}

impl AddonUrl {
    pub fn to_url(&self) -> Url {
        self.url.clone()
    }
}
impl Display for AddonUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let stremio_protocol = self.url.to_string().replace("https://", "stremio://");

        f.write_str(&stremio_protocol)
    }
}

/// Debug printing line as a tuple - `AddonUrl(stremio://....)`
impl Debug for AddonUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("AddonUrl").field(&self.to_string()).finish()
    }
}

pub struct ServerUrl {
    url: Url,
}

pub enum StremioWeb {
    // todo: replace with url
    Addon(AddonUrl),
    OpenWeb { server_url: Option<Url> },
}

impl StremioWeb {
    pub fn open(self) {
        let url_to_open = match self {
            StremioWeb::Addon(addon_url) => addon_url.to_url(),
            StremioWeb::OpenWeb {
                server_url: Some(server_url),
            } => {
                let mut stremio_url = STREMIO_URL.clone();

                let query = format!("streamingServer={}", encode(server_url.as_ref()));

                stremio_url.set_query(Some(&query));
                stremio_url
            }
            StremioWeb::OpenWeb { server_url: None } => STREMIO_URL.clone(),
        };

        match open::that(url_to_open.to_string()) {
            Ok(_) => info!("Opened Stremio Web in the browser: {url_to_open}"),
            Err(e) => error!("Failed to open {url_to_open} in Stremio Web: {}", e),
        }
    }
}
/// Only for Linux and MacOS
#[cfg(all(feature = "bundled", any(target_os = "linux", target_os = "macos")))]
fn make_it_autostart(home_dir: impl AsRef<Path>) {
    #[cfg(target_os = "linux")]
    {
        use crate::{
            config::{AUTOSTART_CONFIG_PATH, DESKTOP_FILE_NAME, DESKTOP_FILE_PATH},
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

            let open_url = match open_url.parse() {
                Ok(addon_url) => StremioWeb::Addon(open_url).open(),
                Err(err) => {
                    error!("{err}");
                    StremioWeb::OpenWeb { server_url: None }.open()
                }
            };
        }),
    );

    app
}
