// Copyright (C) 2017-2023 Smart code 203358507

use std::{path::PathBuf, process::Stdio, sync::Arc};

use anyhow::{anyhow, bail, Context, Error};
use futures::executor::block_on;
use log::{error, info, trace};
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, BufReader},
    process::{Child, ChildStdout, Command},
    sync::{mpsc, Mutex},
};
use url::Url;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Debug, Clone)]
pub struct Server {
    inner: Arc<ServerInner>,
}

#[derive(Debug)]
struct ServerInner {
    pub config: Config,
    pub process: Mutex<OnceCell<Child>>,
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
    pub fn at_dir(directory: PathBuf) -> Result<Self, Error> {
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
    /// - `linux` - returns `ffmpeg-linux` or `ffmpeg` (when `bundled` feature is enabled)
    /// - `macos` returns `ffmpeg-macos` or `ffmpeg` (when `bundled` feature is enabled)
    /// - `windows` returns `ffmpeg-windows.exe`
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
    /// - `linux` - returns `node`
    /// - `macos` returns `node`
    /// - `windows` returns `node.exe`
    ///
    /// If no OS is supplied, [`std::env::consts::OS`] is used.
    ///
    /// # Errors
    ///
    /// If any other OS is supplied, see [`std::env::consts::OS`] for more details.
    pub fn node_bin(operating_system: Option<&str>) -> Result<&'static str, Error> {
        match operating_system.unwrap_or(std::env::consts::OS) {
            "linux" | "macos" => Ok("node"),
            "windows" => Ok("node.exe"),
            os => bail!("Operating system {} is not supported", os),
        }
    }
}

impl Server {
    pub fn new(config: Config) -> Self {
        Server {
            inner: Arc::new(ServerInner {
                config,
                process: Default::default(),
            }),
        }
    }

    pub async fn start(&self) -> Result<(), Error> {
        let mut command = Command::new(&self.inner.config.node);
        #[cfg(target_os = "windows")]
        command.creation_flags(CREATE_NO_WINDOW);

        command
            .env("FFMPEG_BIN", &self.inner.config.ffmpeg)
            .env("FFPROBE_BIN", &self.inner.config.ffprobe)
            .arg(&self.inner.config.server)
            .stdout(Stdio::piped())
            .kill_on_drop(true);

        info!("Starting Server: {:#?}", command);

        let child_process = self.inner.process.lock().await;
        if child_process.get().is_none() {
            match command.spawn() {
                Ok(new_process) => {
                    let process_pid = new_process.id();
                    info!("Server started. (PID {:?})", process_pid);

                    child_process
                        .set(new_process)
                        .expect("Should always be empty, we've just checked after all.")
                }
                Err(err) => {
                    error!("Server didn't start: {err}");

                    bail!("Server didn't start: {err}")
                }
            }
        } else {
            info!("Only 1 instance of server can run for an instance, do nothing.")
        }

        Ok(())
    }

    // TODO: add some retry mechanism
    pub async fn settings(&self) -> anyhow::Result<ServerSettingsResponse> {
        // try https, else, use http
        let https_response = reqwest::get("https://127.0.0.1:11470/settings")
            .await
            .and_then(|response| response.error_for_status());

        let response = match https_response {
            Ok(response) => response,
            Err(err) => {
                error!("Failed to reach server.js with HTTPS due to: {err}");

                let http_response = reqwest::get("http://127.0.0.1:11470/settings")
                    .await
                    .and_then(|response| response.error_for_status());

                match http_response {
                    Ok(response) => response,
                    Err(err) => {
                        error!("Failed to reach server.js with HTTP due to: {err}");

                        bail!("Failed to load server /settings")
                    }
                }
            }
        };

        let status = response.status();
        let text = response.text().await?;
        trace!("Response status {:?}; content: {}", status, text);

        serde_json::from_str::<ServerSettingsResponse>(&text)
            .context("failed to parse server settings response")
    }

    pub async fn stdout(&self) -> Result<ChildStdout, Error> {
        let mut process = self.inner.process.lock().await;

        match process.get_mut() {
            Some(child) => match child.stdout.take() {
                Some(stdout) => Ok(stdout),
                None => bail!("Can get stdout only once per process!"),
            },
            None => bail!("No server is running"),
        }
        // match process
        //     .get_mut()
        //     .and_then(|process| process.stdout.take())
        // {
        //     Some(stdout) => {
        //         let mut string = String::new();
        //         stdout
        //             .read_to_string(&mut string)
        //             .await
        //             .context("Failed ot read stdout string")?;

        //         Ok(string)
        //     }
        //     None => {
        //         bail!("No stdout found")
        //     }
        // }
    }

    pub async fn stop(&mut self) -> anyhow::Result<()> {
        match self.inner.process.lock().await.take() {
            Some(mut child_process) => {
                let id = child_process.id();
                child_process
                    .kill()
                    .await
                    .expect("Failed to stop the server process.");

                match id {
                    Some(pid) => info!("Server was shut down. (PID #{})", pid),
                    None => info!("Server is already shut down"),
                }
            }
            None => info!("Server was not running, do nothing."),
        }

        Ok(())
    }

    /// Can be called only once to spawn a logger task for the server!
    pub fn run_logger(&self, server_url_sender: mpsc::Sender<Url>) {
        let server = self.clone();

        tokio::spawn(async move {
            match server.stdout().await {
                Ok(stdout) => {
                    let mut line_reader = BufReader::new(stdout).lines();
                    // can be called only once!
                    loop {
                        match line_reader.next_line().await {
                            Ok(Some(stdout_line)) => {
                                match stdout_line.strip_prefix("EngineFS server started at ") {
                                    Some(server_url) => {
                                        info!("Server url: {server_url}");
                                        match server_url_sender
                                            .send(
                                                server_url
                                                    .parse::<Url>()
                                                    .expect("Should be valid Url!"),
                                            )
                                            .await
                                        {
                                            Ok(_sent) => {
                                                // do nothing
                                            }
                                            Err(err) => error!("Sending server_url failed: {err}"),
                                        };
                                    }
                                    None => {
                                        // skip
                                    }
                                };

                                // trace!("server startup logs: {logs}");
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

impl Drop for Server {
    fn drop(&mut self) {
        match block_on(self.stop()) {
            Ok(()) => {}
            Err(err) => error!("Failed to stop server on Drop, reason: {err}"),
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
