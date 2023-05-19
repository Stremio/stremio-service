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
    /// nodejs binary path
    ///
    /// Includes the OS-dependent suffix:
    /// - `linux` - `node`
    /// - `macos` - `node`
    /// - `windows` - `node.exe`
    node: PathBuf,
    /// ffmpeg binary path
    ///
    /// - `linux` - `ffmpeg-linux` or `ffmpeg` (when `bundled` feature is enabled)
    /// - `macos` - `ffmpeg-macos` or `ffmpeg` (when `bundled` feature is enabled)
    /// - `windows` - `ffmpeg-windows.exe`
    ffmpeg: PathBuf,
    /// ffprobe binary path
    /// 
    ///     /// - `linux` - `ffprobe-linux64` or `ffprobe` (when `bundled` feature is enabled)
    /// - `macos` - `ffprobe-macos` or `ffprobe` (when `bundled` feature is enabled)
    /// - `windows` - `ffprobe-windows.exe`
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
            let ffprobe = directory.join(Self::ffprobe_bin(None)?);
            let server = directory.join("server.js");

            match (
                node.try_exists().context("Nodejs")?,
                ffmpeg.try_exists().context("ffmpeg")?,
                ffprobe.try_exists().context("ffprobe")?,
                server.try_exists().context("server.js")?,
            ) {
                (true, true, true, true) => Ok(Self {
                    node,
                    ffmpeg,
                    ffprobe,
                    server,
                }),
                (false, true, true, true) => {
                    bail!("Nodejs not found at: {}", node.display().to_string())
                }
                (true, false, true, true) => {
                    bail!("ffmpeg not found at: {}", ffmpeg.display().to_string())
                }
                (true, true, false, true) => {
                    bail!("ffprobe not found at: {}", server.display().to_string())
                }
                (true, true, true, false) => {
                    bail!("server.js not found at: {}", server.display().to_string())
                }
                (false, false, false, false) => bail!(
                    "Nodejs, ffmpeg and server.js not found in directory: {}",
                    directory.display().to_string()
                ),
                _ => {
                    bail!(
                        "More than 1 required binary was not found; paths: {}; {}; {}; {}",
                        node.display().to_string(),
                        ffmpeg.display().to_string(),
                        ffprobe.display().to_string(),
                        server.display().to_string(),
                    )
                }
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
            "linux" => {
                if cfg!(feature = "bundled") {
                    Ok("ffmpeg")
                } else {
                    Ok("ffmpeg-linux")
                }
            }
            "macos" => {
                if cfg!(feature = "bundled") {
                    Ok("ffmpeg")
                } else {
                    Ok("ffmpeg-macos")
                }
            }
            "windows" => Ok("ffmpeg-windows.exe"),
            os => bail!("Operating system {} is not supported", os),
        }
    }

    pub fn ffprobe_bin(operating_system: Option<&str>) -> Result<&'static str, Error> {
        match operating_system.unwrap_or(std::env::consts::OS) {
            "linux" => {
                if cfg!(feature = "bundled") {
                    Ok("ffprobe")
                } else {
                    Ok("ffprobe-linux64")
                }
            }
            "macos" => {
                if cfg!(feature = "bundled") {
                    Ok("ffprobe")
                } else {
                    Ok("ffprobe-macos")
                }
            }
            "windows" => Ok("ffprobe-windows.exe"),
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
        #[cfg(target_os = "windows")]
        command.creation_flags(CREATE_NO_WINDOW);
        command.env("FFMPEG_BIN", &self.inner.config.ffmpeg);
        dbg!(&self.inner.config.ffprobe);
        command.env("FFPROBE_BIN", &self.inner.config.ffprobe);
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
