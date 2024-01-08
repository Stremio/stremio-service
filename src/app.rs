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
use log::{debug, error, info};
use rand::Rng;
use reqwest::StatusCode;
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};
use tao::{
    event::Event,
    event_loop::{ControlFlow, EventLoop, EventLoopProxy},
};
use tokio::{
    select,
    sync::watch,
    time::{interval_at, Instant},
};
use url::Url;
use urlencoding::encode;

use crate::{
    app::tray_menu::ServerAction,
    cli::Cli,
    constants::{STREMIO_URL, UPDATE_ENDPOINT},
    server::{self, Info, Server, DEFAULT_SERVER_URL},
    updater::Updater,
};

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
    server_js: Option<ServerTrayStatus>,
}

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
#[allow(clippy::large_enum_variant)]
enum ServerTrayStatus {
    #[default]
    Stopped,
    /// The server is currently being restarted
    Restarting,
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
    // data_dir: PathBuf,

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
        cli: Cli,
        home_dir: PathBuf,
        cache_dir: PathBuf,
        service_bins_dir: PathBuf,
    ) -> Result<Self, Error> {
        let server = server::Config::at_dir(service_bins_dir, cli.no_cors)
            .context("Server configuration failed")?;

        let lockfile = cache_dir.join("lock");

        let updater_endpoint = if let Some(endpoint) = cli.updater_endpoint {
            endpoint
        } else {
            let mut url = Url::parse(Self::get_random_updater_endpoint().as_str())?;
            if cli.release_candidate {
                url.query_pairs_mut().append_pair("rc", "true");
            }
            url
        };

        Ok(Self {
            updater_endpoint,
            home_dir,
            lockfile,
            server,
            skip_update: cli.skip_updater,
            force_update: cli.force_update,
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
    pub const DEV_SERVER_STATUS_EVERY: Duration = Duration::from_secs(3);

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

        // we manually start the server the first time, to make sure it starts with the app without any delay
        // in child process start nor in updating the server status in the Tray menu
        let server_info = self
            .server
            .start()
            .await
            .context("Failed to start Server")?;

        info!("Server started successfully: {server_info:#?}");
        // self.server.run_logger(server_url_sender);

        let (action_sender, action_receiver) = tokio::sync::watch::channel(None);
        let (status_sender, status_receiver) =
            tokio::sync::watch::channel(Some(ServerTrayStatus::Running {
                info: server_info.clone(),
            }));

        tokio::spawn(Self::run_server_process_updater(
            action_receiver,
            status_sender,
            self.server.clone(),
        ));

        let tray_status = TrayStatus {
            server_js: Some(ServerTrayStatus::Running {
                info: Info {
                    config: server_info.config.clone(),
                    version: server_info.version,
                    base_url: server_info.base_url,
                },
            }),
        };

        let mut tray_menu = TrayMenu::new(&event_loop)?;
        tray_menu.set_status(tray_status);

        let stats_updater = Self::run_tray_status_updater(
            self.server.clone(),
            status_receiver,
            event_loop.create_proxy(),
        );
        tokio::spawn(stats_updater);

        let server_url_receiver = self.server.server_url_receiver();
        event_loop.run(move |event, _event_loop, control_flow| {
            *control_flow = ControlFlow::Wait;

            match event {
                Event::MenuEvent { menu_id, .. } if menu_id == *OPEN_MENU => {
                    // no need to pass the url to stremio-web if it's the default one.
                    let server_url =
                        server_url_receiver
                            .borrow()
                            .to_owned()
                            .and_then(|server_url| {
                                info!("Server url from watch Channel: {}", server_url);
                                if *DEFAULT_SERVER_URL == server_url {
                                    None
                                } else {
                                    Some(server_url)
                                }
                            });

                    let web_url = futures::executor::block_on(Self::detect_web_url());

                    StremioWeb::OpenWeb {
                        server_url,
                        web_url,
                    }
                    .open()
                }
                Event::MenuEvent { menu_id, .. } if menu_id == *QUIT_MENU => {
                    *control_flow = ControlFlow::Exit;
                }
                Event::LoopDestroyed => {
                    // Server will be stopped on drop!
                    // do nothing
                }
                Event::MenuEvent { menu_id, .. } if menu_id == *START_SERVER_MENU => {
                    // unset the server to disable the tray menu items.
                    tray_menu.unset_server();

                    if let Err(err) = action_sender.send(Some(ServerAction::Start)) {
                        error!("Start server action: {err}")
                    }
                }
                Event::MenuEvent { menu_id, .. } if menu_id == *STOP_SERVER_MENU => {
                    // unset the server to disable the tray menu items.
                    tray_menu.unset_server();

                    if let Err(err) = action_sender.send(Some(ServerAction::Stop)) {
                        error!("Stop server action: {err}")
                    }
                }
                Event::MenuEvent { menu_id, .. } if menu_id == *RESTART_SERVER_MENU => {
                    // unset the server to disable the tray menu items.
                    tray_menu.unset_server();

                    if let Err(err) = action_sender.send(Some(ServerAction::Restart)) {
                        error!("Restart server action: {err}")
                    }
                }
                Event::UserEvent(menu_event) => match menu_event {
                    MenuEvent::UpdateTray(new_tray) => tray_menu.set_status(new_tray),
                },
                _ => (),
            }
        });
    }

    pub(crate) fn update_every() -> Duration {
        #[cfg(debug_assertions)]
        let duration = Self::DEV_SERVER_STATUS_EVERY;
        #[cfg(not(debug_assertions))]
        let duration = Self::SERVER_STATUS_EVERY;

        duration
    }

    /// Detect custom web url
    ///
    /// At this point just see if the Dev server returns successful response
    pub async fn detect_web_url() -> Option<Url> {
        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .build()
            .ok()?;

        match client
            .get(crate::constants::DEV_STREMIO_URL.to_owned())
            .send()
            .await
            .and_then(|response| response.error_for_status())
        {
            Ok(response) if response.status() == StatusCode::OK => {
                Some(crate::constants::DEV_STREMIO_URL.to_owned())
            }
            Ok(_) => None,
            Err(err) => {
                error!("{err}");

                None
            }
        }
    }

    async fn run_tray_status_updater(
        server: Server,
        mut status_receiver: watch::Receiver<Option<ServerTrayStatus>>,
        event_loop_proxy: EventLoopProxy<MenuEvent>,
    ) {
        let mut interval = interval_at(Instant::now() + Self::update_every(), Self::update_every());

        loop {
            let status = select! {
                _instant = interval.tick() => {
                    let info = server.update_status().await;

                    let status = info.map(|info| {
                        ServerTrayStatus::Running { info: info.clone() }
                    })
                    // keep the latest status, this will ensure that when processing actions
                    // like Stopping the server, we can know if we are transitioning from states.
                    .or_else(|| status_receiver.borrow().clone());

                    debug!("Server status is updated (every {}s)", Self::update_every().as_secs());

                    status
                },
                Ok(_) = status_receiver.changed() => {
                    let status = status_receiver.borrow().clone();
                    debug!("Server status was updated by an action");

                    status
                }
            };
            let tray_status = TrayStatus { server_js: status };

            match event_loop_proxy.send_event(MenuEvent::UpdateTray(tray_status)) {
                Ok(_) => {
                    // do nothing
                }
                Err(err) => error!("Failed to send new status for tray menu. {err}"),
            }
        }
    }

    /// This updater makes sure that we don't block the event loop of `tao`
    /// when updating (like restarting, stopping and starting) the server process.
    async fn run_server_process_updater(
        mut receiver: watch::Receiver<Option<ServerAction>>,
        status_sender: watch::Sender<Option<ServerTrayStatus>>,
        server: Server,
    ) {
        // errors only when sender is dropped.
        while receiver.changed().await.is_ok() {
            let action = *receiver.borrow();
            info!("Action initialised: {action:?}");

            let action = match action {
                Some(action) => action,
                None => {
                    // we don't care about the initial value of None
                    // because we've manually started the server
                    continue;
                }
            };

            // first make sure to update the status to None while we perform any of the actions
            if let Err(_err) = status_sender.send(None) {
                error!("Failed to update Server tray status on stop because receiver was already closed")
            }

            match action {
                ServerAction::Start => {
                    // handle the result of starting the server
                    match server.start().await {
                        Ok(info) => {
                            // handle the result of closed channel receiver
                            if let Err(_err) =
                                status_sender.send(Some(ServerTrayStatus::Running { info }))
                            {
                                error!("Failed to update Server tray status on stop because receiver was already closed");
                            };
                        }
                        Err(err) => error!("Failed to start Server: {err}"),
                    }
                }
                ServerAction::Stop => {
                    // handle the result of stopping the server
                    if let Err(err) = server.stop().await {
                        error!("Failed to stop Server: {err}");
                    }

                    // Even if stopping fails, then probably the process isn't running any more,
                    // we can safely assume it's in a Stopped state.
                    // handle the result of closed channel receiver
                    if let Err(_err) = status_sender.send(Some(ServerTrayStatus::Stopped)) {
                        error!("Failed to update Server tray status on stop because receiver was already closed")
                    }
                }
                ServerAction::Restart => {
                    // handle the result of closed channel receiver
                    if let Err(_err) = status_sender.send(Some(ServerTrayStatus::Restarting)) {
                        error!("Failed to update Server tray status on beginning of restart because receiver was already closed")
                    }

                    info!("Server restarting...");

                    // handle the result of restarting the server
                    let status = match server.restart().await {
                        Ok(info) => {
                            info!("Server restarted successfully: {info:#?}");
                            ServerTrayStatus::Running { info }
                        }
                        Err(err) => {
                            error!("Failed to restart Server: {err}");
                            ServerTrayStatus::Stopped
                        }
                    };

                    // handle the result of closed channel receiver
                    if let Err(_err) = status_sender.send(Some(status)) {
                        error!("Failed to update Server tray status on end of restart because receiver was already closed");
                    };
                }
            };
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

pub enum StremioWeb {
    Addon(AddonUrl),
    OpenWeb {
        /// Defaults to [`STREMIO_URL`] if not passed.
        web_url: Option<Url>,
        server_url: Option<Url>,
    },
}

impl StremioWeb {
    pub fn open(self) {
        let url_to_open = match self {
            StremioWeb::Addon(addon_url) => addon_url.to_url(),
            StremioWeb::OpenWeb {
                web_url,
                server_url,
            } => {
                let mut web_url = web_url.unwrap_or(STREMIO_URL.to_owned());

                if let Some(server_url) = server_url {
                    let query = format!("streamingServer={}", encode(server_url.as_ref()));

                    web_url.set_query(Some(&query));
                }

                web_url
            }
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

            match open_url.parse::<AddonUrl>() {
                Ok(addon_url) => StremioWeb::Addon(addon_url).open(),
                Err(err) => {
                    // no need to pass the url to stremio-web if it's the default one.
                    let server_url =
                        server_url_receiver
                            .borrow()
                            .to_owned()
                            .and_then(|server_url| {
                                info!("Server url from watch Channel: {}", server_url);
                                if *DEFAULT_SERVER_URL == server_url {
                                    None
                                } else {
                                    Some(server_url)
                                }
                            });
                    error!("Error parsing addon url for schema event! {err}");
                    warn!("Open stremio web instead...");
                    StremioWeb::OpenWeb {
                        server_url,
                        web_url: futures::executor::block_on(Self::detect_web_url()),
                    }
                    .open()
                }
            };
        }),
    );

    app
}
