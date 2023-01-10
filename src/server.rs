use anyhow::{anyhow, bail, Error};
use log::{error, info};
use once_cell::sync::OnceCell;
use std::{
    path::PathBuf,
    process::{Child, Command},
    sync::{Arc, Mutex},
};

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
    /// nodejs binary path
    ///
    /// Includes the OS-dependent suffix:
    /// - `linux` - `node`
    /// - `macos` - `node`
    /// - `windows` - `node.exe`
    node: PathBuf,
    /// ffmpeg binary path
    ///
    /// Includes the OS-dependent suffix:
    /// - `linux` - `ffmpeg-linux`
    /// - `macos` - `ffmpeg-macos`
    /// - `windows` - `ffmpeg-windows.exe`
    ffmpeg: PathBuf,
    /// server.js binary path
    server: PathBuf,
}

impl Config {
    /// Create a Config using the same directory for all binaries
    ///
    /// The directory should contain the following binaries:
    ///
    /// - node(.exe) - depending on target OS being `windows` or not.
    /// - ffmpeg(-linux | -macos | -windows.exe) - depending on the target OS.
    /// - server.js
    ///
    /// # Errors
    ///
    /// When one of the binaries required for running the server is missing.
    pub fn at_dir(directory: PathBuf) -> Result<Self, Error> {
        if directory.is_dir() {
            let node = directory.join(Self::node_bin(None)?);
            let ffmpeg = directory.join(Self::ffmpeg_bin(None)?);
            let server = directory.join("server.js");

            match (node.exists(), ffmpeg.exists(), server.exists()) {
                (false, true, true) => bail!("Nodejs not found at: {}", node.display().to_string()),
                (true, false, true) => {
                    bail!("ffmpeg not found at: {}", ffmpeg.display().to_string())
                }
                (true, true, false) => {
                    bail!("server.js not found at: {}", server.display().to_string())
                }
                (false, false, false) => bail!(
                    "Nodejs, ffmpeg and server.js not found in directory: {}",
                    directory.display().to_string()
                ),
                _ => Ok(Self {
                    node,
                    ffmpeg,
                    server,
                }),
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
    /// - `linux` - returns `ffmpeg-linux`
    /// - `macos` returns `ffmpeg-macos`
    /// - `windows` returns `ffmpeg-windows.exe`
    ///
    /// If no OS is supplied, [`std::env::consts::OS`] is used.
    ///
    /// # Errors
    ///
    /// If any other OS is supplied, see [`std::env::consts::OS`] for more details.
    pub fn ffmpeg_bin(operating_system: Option<&str>) -> Result<&'static str, Error> {
        match operating_system.unwrap_or(std::env::consts::OS) {
            "linux" => Ok("ffmpeg-linux"),
            "macos" => Ok("ffmpeg-macos"),
            "windows" => Ok("ffmpeg-windows.exe"),
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
            "linux" => Ok("node"),
            "macos" => Ok("node"),
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

    pub fn start(&self) -> Result<(), Error> {
        let mut command = Command::new(&self.inner.config.node);
        command.env("FFMPEG_BIN", &self.inner.config.ffmpeg);
        command.arg(&self.inner.config.server);

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
