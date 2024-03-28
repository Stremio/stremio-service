// Copyright (C) 2017-2024 Smart Code OOD 203358507

use anyhow::{anyhow, bail, Error};
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
    server: PathBuf,
    node: PathBuf,
    ffmpeg: PathBuf,
    ffprobe: PathBuf,
}

impl Config {
    /// Create a Config using the same directory for all binaries
    ///
    /// # Errors
    ///
    /// When one of the binaries required for running the server is missing.
    pub fn new(directory: PathBuf) -> Result<Self, Error> {
        if directory.is_dir() {
            let server = directory.join("server.js");
            let node = directory.join(Self::node_bin()?);
            let ffmpeg = directory.join(Self::ffmpeg_bin()?);
            let ffprobe = directory.join(Self::ffprobe_bin()?);

            let binaries_paths = [
                server.clone(),
                node.clone(),
                ffmpeg.clone(),
                ffprobe.clone(),
            ];

            for path in binaries_paths.iter() {
                if !path.exists() {
                    bail!("Failed to locate the file {:?}", path)
                }
            }

            Ok(Self {
                server,
                node,
                ffmpeg,
                ffprobe,
            })
        } else {
            bail!(
                "The path '{:?}' does not exist or it is not a directory",
                directory
            )
        }
    }
    fn node_bin() -> Result<&'static str, Error> {
        match std::env::consts::OS {
            "linux" | "macos" => Ok("stremio-runtime"),
            "windows" => Ok("stremio-runtime.exe"),
            os => bail!("Operating system {} is not supported", os),
        }
    }
    fn ffmpeg_bin() -> Result<&'static str, Error> {
        match std::env::consts::OS {
            "linux" | "macos" => Ok("ffmpeg"),
            "windows" => Ok("ffmpeg.exe"),
            os => bail!("Operating system {} is not supported", os),
        }
    }
    fn ffprobe_bin() -> Result<&'static str, Error> {
        match std::env::consts::OS {
            "linux" | "macos" => Ok("ffprobe"),
            "windows" => Ok("ffprobe.exe"),
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
