// Copyright (C) 2017-2024 Smart Code OOD 203358507

use std::{path::PathBuf, process::Stdio, sync::Arc, time::Duration};

use anyhow::{anyhow, bail, Context, Error};
use futures::executor::block_on;
use futures_util::TryFutureExt;
use log::{error, info, trace};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, ChildStdout, Command},
    sync::{watch, RwLock},
    time::sleep,
};
use url::Url;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

// TODO: make configurable
/// Wait 3 seconds for the server to start
const WAIT_AFTER_START: Duration = Duration::from_secs(3);

/// Waits 6 seconds for the server to fully stop
const WAIT_FOR_FULL_STOP: Duration = Duration::from_secs(6);

pub static DEFAULT_SERVER_URL: Lazy<Url> = Lazy::new(|| "http://127.0.0.1:11470".parse().unwrap());

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Info {
    pub config: Config,
    /// Not prefixed with `v`, taken from server.js `/settings`
    ///
    /// # Examples
    /// - "4.20.0"
    /// - "4.20.1"
    /// - "4.20.2"
    /// - etc.
    pub version: String,

    /// The server base url in the local network.
    ///
    /// # Examples:
    ///
    /// - `http://192.168.0.215:11470`
    pub base_url: Url,
}

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum ServerStatus {
    Stopped,
    Running { process: Child, info: Info },
}

impl Default for ServerStatus {
    fn default() -> Self {
        Self::Stopped
    }
}

impl ServerStatus {
    pub fn stopped() -> Self {
        Self::Stopped
    }

    pub fn running(info: Info, process: Child) -> ServerStatus {
        Self::Running { process, info }
    }
}

#[derive(Debug, Clone)]
pub struct Server {
    inner: Arc<ServerInner>,
}

impl Server {
    pub fn new(config: Config) -> Server {
        let (server_url_sender, server_url_receiver) = tokio::sync::watch::channel(None);

        Server {
            inner: Arc::new(ServerInner {
                config,
                status: RwLock::new(ServerStatus::Stopped),
                server_url_sender,
                server_url_receiver,
            }),
        }
    }

    /// Starts the server if it is in a stopped state ( [`ServerStatus::Stopped`] )
    pub async fn start(&self) -> Result<Info, Error> {
        // if status is running return early and release read lock
        {
            if let ServerStatus::Running { process, info } = &*self.inner.status.read().await {
                info!(
                    "Server is already running (PID: {})",
                    process.id().unwrap_or_default()
                );

                return Ok(info.clone());
            }
        }

        // otherwise, start a new process and only at the end override the status
        let info = {
            let mut command = Command::new(&self.inner.config.node);
            #[cfg(target_os = "windows")]
            command.creation_flags(CREATE_NO_WINDOW);

            if self.inner.config.no_cors {
                command.env("NO_CORS", "1");
            }

            command
                .env("FFMPEG_BIN", &self.inner.config.ffmpeg)
                .env("FFPROBE_BIN", &self.inner.config.ffprobe)
                .arg(&self.inner.config.server)
                .stdout(Stdio::piped())
                .kill_on_drop(true);

            info!("Starting Server: {:#?}", command);

            match command.spawn() {
                Ok(mut new_process) => {
                    let process_pid = new_process.id();
                    info!("Server started. (PID {:?})", process_pid);

                    // wait given amount of time to make sure the server has started up and is running
                    sleep(WAIT_AFTER_START).await;

                    let std_out = new_process
                        .stdout
                        .take()
                        .ok_or(anyhow!("Couldn't retrieve stdout of child process"))?;

                    // TODO: return or set the std_out in the Server for observing the log
                    let server_inner = self.inner.clone();
                    tokio::spawn(async move {
                        let mut line_reader = BufReader::new(std_out).lines();
                        // can be called only once!
                        loop {
                            match line_reader.next_line().await {
                                Ok(Some(stdout_line)) => {
                                    if let Some(server_url) =
                                        stdout_line.strip_prefix("EngineFS server started at ")
                                    {
                                        match server_url.parse::<Url>() {
                                            Ok(server_url) => {
                                                info!("Server url: {server_url}");
                                                server_inner.server_url_sender.send_replace(Some(server_url));
                                            }
                                            Err(err) => error!(
                                                "Error when passing {server_url} as server url: {err}"
                                            ),
                                        }
                                    }
                                    // TODO: Feature Server logging
                                }
                                Ok(None) => {
                                    // do nothing
                                }
                                Err(err) => error!("Error collecting Server logs: {err}"),
                            }
                        }
                    });

                    // `http` url for settings endpoint is always available
                    let url = {
                        let mut url_receiver = self.inner.server_url_receiver.clone();
                        let mut base_url: Url;
                        // wait for the value to get changed
                        loop {
                            match url_receiver.changed().await {
                                Ok(_) => {
                                    if let Some(url) =
                                        self.inner.server_url_receiver.borrow().as_ref()
                                    {
                                        base_url = url.clone();
                                        break;
                                    }
                                }
                                Err(err) => {
                                    error!("{err}");
                                    continue;
                                }
                            }
                        }
                        base_url.set_scheme("http").unwrap();

                        base_url
                    };
                    let settings = Self::settings(url).await?;

                    let info = Info {
                        config: self.inner.config.clone(),
                        version: settings.values.server_version,
                        base_url: settings.base_url,
                    };
                    // set new child process
                    *self.inner.status.write().await =
                        ServerStatus::running(info.clone(), new_process);
                    info!("Server has started and new process is set");

                    info
                }
                Err(err) => {
                    error!("Server didn't start: {err}");

                    bail!("Server didn't start: {err}")
                }
            }
        };

        Ok(info)
    }

    // TODO: add some retry mechanism
    pub async fn settings(server_url: Url) -> anyhow::Result<ServerSettingsResponse> {
        let response = reqwest::get(server_url.join("/settings").unwrap())
            .await
            .and_then(|response| response.error_for_status());

        match response {
            Ok(response) => {
                let status = response.status();
                let text = response.text().await?;
                trace!("Response status {:?}; content: {}", status, text);

                serde_json::from_str::<ServerSettingsResponse>(&text)
                    .context("failed to parse server settings response")
            }
            Err(err) => {
                error!("Failed to reach server.js with HTTP due to: {err}");

                bail!("Failed to load server /settings")
            }
        }
    }

    pub async fn stdout(&self) -> Result<ChildStdout, Error> {
        match &mut *self.inner.status.write().await {
            ServerStatus::Stopped => bail!("Server is not running"),
            ServerStatus::Running { process, .. } => match process.stdout.take() {
                Some(stdout) => Ok(stdout),
                None => bail!("Can get stdout only once per process!"),
            },
        }
    }

    pub fn server_url_receiver(&self) -> watch::Receiver<Option<Url>> {
        self.inner.server_url_receiver.clone()
    }

    /// Checks if the child process is still running and returns the information about the server configuration if it does.
    ///
    /// If the child process has exited for some reason, this method returns `None`
    /// and you have to run `start()` again.
    pub async fn update_status(&self) -> Option<Info> {
        let (new_status, info) = {
            match &*self.inner.status.read().await {
                ServerStatus::Running { process, info } => {
                    let is_running = process.id();

                    if is_running.is_some() {
                        (None, Some(info.clone()))
                    } else {
                        info!("Child process of the server has exited");

                        (Some(ServerStatus::Stopped), None)
                    }
                }
                ServerStatus::Stopped => {
                    info!("update_status: Server hasn't been started yet, do nothing.");

                    (None, None)
                }
            }
        };

        if let Some(new_status) = new_status {
            *self.inner.status.write().await = new_status;
        }

        info
    }

    /// Stops the server it's currently running ( [`ServerStatus::Running`] )
    ///
    /// # Returns
    /// `true` if there was a server PID that was stopped
    /// `false` if the process has already been killed before this call
    pub async fn stop(&self) -> anyhow::Result<bool> {
        self.inner.stop().await
    }

    pub async fn restart(&self) -> anyhow::Result<Info> {
        match self.stop().await {
            Ok(_) => {}
            // no need to wait if server stopping returned an error
            Err(err) => error!("Restarting (stop): {err}"),
        }

        self.start()
            .inspect_err(|err| error!("Restarting (start): {err}"))
            .await
    }

    #[deprecated]
    /// Can be called only once to spawn a logger task for the server!
    pub fn run_logger(&self, url_sender: watch::Sender<Option<Url>>) {
        let server = self.clone();

        tokio::spawn(async move {
            match server.stdout().await {
                Ok(stdout) => {
                    let mut line_reader = BufReader::new(stdout).lines();
                    // can be called only once!
                    loop {
                        match line_reader.next_line().await {
                            Ok(Some(stdout_line)) => {
                                if let Some(server_url) =
                                    stdout_line.strip_prefix("EngineFS server started at ")
                                {
                                    match server_url.parse::<Url>() {
                                        Ok(server_url) => {
                                            info!("Server url: {server_url}");
                                            url_sender.send_replace(Some(server_url));
                                        }
                                        Err(err) => error!(
                                            "Error when passing {server_url} as server url: {err}"
                                        ),
                                    }
                                }
                            }
                            Ok(None) => {
                                // do nothing
                            }
                            Err(err) => error!("Error collecting Server logs: {err}"),
                        }
                    }
                }
                Err(err) => error!("{err}"),
            }
        });
    }
}

#[derive(Debug)]
struct ServerInner {
    pub config: Config,
    pub status: RwLock<ServerStatus>,
    server_url_sender: watch::Sender<Option<Url>>,
    pub server_url_receiver: watch::Receiver<Option<Url>>,
}

impl ServerInner {
    pub async fn stop(&self) -> anyhow::Result<bool> {
        let mut status = self.status.write().await;

        match &mut *status {
            ServerStatus::Running { process, .. } => {
                let id = process.id();
                let kill_result = process
                    .kill()
                    .await
                    .context("Failed to stop the server process.");

                let pid_stopped = match id {
                    // if the server process was just killed only then wait till it's fully stopped.
                    Some(pid) => {
                        info!("Server was shut down. (PID #{})", pid);
                        // wait for the server to fully stop
                        sleep(WAIT_FOR_FULL_STOP).await;
                        info!(
                            "Waited {} seconds for the server to fully stop",
                            WAIT_FOR_FULL_STOP.as_secs()
                        );

                        true
                    }
                    None => {
                        info!("Server is already shut down");
                        false
                    }
                };

                kill_result.map(|_| pid_stopped)
            }
            ServerStatus::Stopped => {
                info!("stop: Server hasn't been started yet, do nothing.");
                Ok(false)
            }
        }
    }
}

impl Drop for ServerInner {
    fn drop(&mut self) {
        match block_on(self.stop()) {
            Ok(_) => {}
            Err(err) => error!("Failed to stop server on Drop, reason: {err}"),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerSettingsResponse {
    pub values: SettingsValues,
    pub base_url: Url,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsValues {
    pub server_version: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    /// Full `nodejs` binary path
    ///
    /// Includes the OS-dependent suffix:
    /// - `linux` - `node`
    /// - `macos` - `node`
    /// - `windows` - `node.exe`
    node: PathBuf,
    /// Full `ffmpeg` binary path
    ///
    /// - `linux` - `ffmpeg`
    /// - `macos` - `ffmpeg`
    /// - `windows` - `ffmpeg.exe`
    ffmpeg: PathBuf,
    /// Full `ffprobe` binary path
    ///
    /// - `linux` - `ffprobe`
    /// - `macos` - `ffprobe`
    /// - `windows` - `ffprobe.exe`
    ffprobe: PathBuf,
    /// server.js binary path
    server: PathBuf,
    /// Whether or not to set the `NO_CORS` environment variable to `1`
    /// and disable the CORS checks in the Server.
    no_cors: bool,
}

impl Config {
    /// Create a Config using the same directory for all binaries
    ///
    /// The directory should contain the following binaries:
    ///
    /// - node(.exe) - depending on target OS being `windows` or not.
    /// - ffmpeg(.exe) - depending on target OS being `windows` or not.
    /// - server.js
    ///
    /// # Errors
    ///
    /// When one of the binaries required for running the server is missing.
    pub fn at_dir(directory: PathBuf, no_cors: bool) -> Result<Self, Error> {
        if directory.is_dir() {
            let node = directory.join(Self::node_bin(None)?);
            let server = directory.join("server.js");

            let ffmpeg = directory.join(Self::ffmpeg_bin(None)?);
            let ffprobe = directory.join(Self::ffprobe_bin(None)?);

            let node_exists = node.try_exists().context("Nodejs").map(|exists| {
                if !exists {
                    bail!("Nodejs not found at: {}", node.display().to_string())
                } else {
                    Ok(())
                }
            })?;

            let ffmpeg_exists = ffmpeg.try_exists().context("ffmpeg").map(|exists| {
                if !exists {
                    bail!("ffmpeg not found at: {}", ffmpeg.display().to_string())
                } else {
                    Ok(())
                }
            })?;

            let ffprobe_exists = ffprobe.try_exists().context("ffprobe").map(|exists| {
                if !exists {
                    bail!("ffprobe not found at: {}", server.display().to_string())
                } else {
                    Ok(())
                }
            })?;
            let server_exists = ffprobe.try_exists().context("server.js").map(|exists| {
                if !exists {
                    bail!("server.js not found at: {}", server.display().to_string())
                } else {
                    Ok(())
                }
            })?;

            let binaries_exist = vec![node_exists, ffmpeg_exists, ffprobe_exists, server_exists];

            // we have at least 1 missing binary
            if binaries_exist.iter().any(|result| result.is_err()) {
                bail!(
                    "One or more binaries were not found; paths: {}; {}; {}; {}; Errors: {:?}",
                    node.display().to_string(),
                    ffmpeg.display().to_string(),
                    ffprobe.display().to_string(),
                    server.display().to_string(),
                    binaries_exist
                        .iter()
                        .filter_map(|result| match result {
                            Ok(()) => None,
                            Err(err) => Some(err),
                        })
                        .collect::<Vec<_>>()
                );
            } else {
                Ok(Self {
                    node,
                    ffmpeg,
                    ffprobe,
                    server,
                    no_cors,
                })
            }
        } else {
            bail!(
                "The path '{}' does not exist or it is not a directory",
                directory.display().to_string()
            )
        }
    }

    /// Returns the ffmpeg binary name (Operating system dependent).
    ///
    /// Supports only 3 OSes:
    /// - `linux` - returns `ffmpeg`
    /// - `macos` returns `ffmpeg`
    /// - `windows` returns `ffmpeg.exe`
    ///
    /// If no OS is supplied, [`std::env::consts::OS`] is used.
    ///
    /// # Errors
    ///
    /// If any other OS is supplied, see [`std::env::consts::OS`] for more details.
    pub fn ffmpeg_bin(operating_system: Option<&str>) -> Result<&'static str, Error> {
        match operating_system.unwrap_or(std::env::consts::OS) {
            "linux" | "macos" => Ok("ffmpeg"),
            "windows" => Ok("ffmpeg.exe"),
            os => bail!("Operating system {} is not supported", os),
        }
    }

    pub fn ffprobe_bin(operating_system: Option<&str>) -> Result<&'static str, Error> {
        match operating_system.unwrap_or(std::env::consts::OS) {
            "linux" | "macos" => Ok("ffprobe"),
            "windows" => Ok("ffprobe.exe"),
            os => bail!("Operating system {} is not supported", os),
        }
    }

    /// Returns the node binary name (Operating system dependent).
    ///
    /// Supports only 3 OSes:
    /// - `linux` - returns `stremio-runtime`
    /// - `macos` - returns `stremio-runtime`
    /// - `windows` - returns `stremio-runtime.exe`
    ///
    /// If no OS is supplied, [`std::env::consts::OS`] is used.
    ///
    /// # Errors
    ///
    /// If any other OS is supplied, see [`std::env::consts::OS`] for more details.
    pub fn node_bin(operating_system: Option<&str>) -> Result<&'static str, Error> {
        match operating_system.unwrap_or(std::env::consts::OS) {
            "linux" | "macos" => Ok("stremio-runtime"),
            "windows" => Ok("stremio-runtime.exe"),
            os => bail!("Operating system {} is not supported", os),
        }
    }
}

#[cfg(test)]
mod test {
    use super::Server;

    fn is_sync<T: Sync>() {}
    fn is_send<T: Send>() {}

    #[test]
    fn test_server_sync_and_send() {
        is_sync::<Server>();
        is_send::<Server>();
    }
}
