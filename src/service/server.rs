use std::{fs, error::Error, process::{Child, Command}, path::PathBuf};
use log::{error, info};

use stremio_service::shared::join_current_exe_dir;

const STREMIO_SERVER_URL: &str = "https://dl.strem.io/four/master/server.js";


pub struct Server {
    data_location: PathBuf,
    server_location: PathBuf,
    pub process: Option<Child>
}

impl Server {
    pub fn new(data_location: PathBuf) -> Self {
        let server_location = data_location.join("server.js");

        Self {
            data_location,
            server_location,
            process: None
        }
    }

    pub async fn update(&self) -> Result<(), Box<dyn Error>> {
        let server_js_file = reqwest::get(STREMIO_SERVER_URL)
            .await?
            .text()
            .await?;
        
        fs::create_dir_all(self.data_location.clone())?;
        fs::write(self.server_location.clone(), server_js_file)?;

        Ok(())
    }

    pub fn start(&mut self) -> Result<(), Box<dyn Error>> {
        let node_binary_path = join_current_exe_dir("node");
        let ffmpeg_binary_path = join_current_exe_dir("ffmpeg");

        let mut command = Command::new(node_binary_path);
        command.env("FFMPEG_BIN", ffmpeg_binary_path);
        command.arg(self.server_location.clone());

        match command.spawn() {
            Ok(process) => {
                let process_pid = process.id();
                info!("Server started. (PID {:?})", process_pid);
                self.process = Some(process);
            },
            Err(err) => error!("Server couldn't be started: {err}")
        }

        Ok(())
    }

    pub fn stop(&mut self) {
        let process = self.process.as_mut().unwrap();
        process.kill()
            .expect("Failed to stop the server process.");
        info!("Server closed. (PID {:?})", process.id());
    }
}