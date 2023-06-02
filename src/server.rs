// Copyright (C) 2017-2023 Smart code 203358507

use anyhow::{anyhow, bail, Context, Error};
use log::{error, info};
use once_cell::sync::OnceCell;
use std::{
    path::PathBuf,
    process::{Child, Command},
    sync::{Arc, Mutex},
};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
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

#[derive(Debug, Clone)]
pub struct Config {
    /// Full `nodejs` binary path
    ///
    /// Includes the OS-dependent suffix:
    /// - `linux` - `stremio-runtime`
    /// - `macos` - `stremio-runtime`
    /// - `windows` - `stremio-runtime.exe`
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
    /// - stremio-runtime(.exe) - depending on target OS being `windows` or not.
    /// - ffmpeg(-linux | -macos | -windows.exe) - depending on the target OS.
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

            let node_exists = node.try_exists().context("stremio runtime").map(|exists| {
                if !exists {
                    bail!("stremio runtime not found at: {}", node.display().to_string())
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
    /// - `linux` returns `ffmpeg`
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
    /// - `macos` returns `stremio-runtime`
    /// - `windows` returns `stremio-runtime.exe`
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

impl Server {
    pub fn new(config: Config) -> Self {
        Server {
            inner: Arc::new(ServerInner {
                config,
                process: Default::default(),
            }),
        }
    }

    pub fn start(&self) -> Result<(), Error> {
        let mut command = Command::new(&self.inner.config.node);
        #[cfg(target_os = "windows")]
        command.creation_flags(CREATE_NO_WINDOW);
        command.env("FFMPEG_BIN", &self.inner.config.ffmpeg);
        command.env("FFPROBE_BIN", &self.inner.config.ffprobe);
        command.arg(&self.inner.config.server);

        info!("Starting server.js: {:#?}", command);

        if self
            .inner
            .process
            .lock()
            .map_err(|_| anyhow!("Failed to lock server.js child process"))?
            .get()
            .is_none()
        {
            match command.spawn() {
                Ok(new_process) => {
                    let process_pid = new_process.id();
                    info!("Server started. (PID {:?})", process_pid);

                    self.inner
                        .process
                        .lock()
                        .map_err(|_| anyhow!("Failed to lock server.js child process"))?
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

    pub fn stop(&mut self) -> Result<(), Error> {
        match self
            .inner
            .process
            .lock()
            .map_err(|_| anyhow!("Failed to lock server.js child process"))?
            .take()
        {
            Some(mut child_process) => {
                child_process
                    .kill()
                    .expect("Failed to stop the server process.");
                info!("Server was shut down. (PID #{})", child_process.id());
            }
            None => info!("Server was not running, do nothing."),
        }

        Ok(())
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        match self.stop() {
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
