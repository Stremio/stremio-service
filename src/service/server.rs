use std::{error::Error, process::{Child, Command}};
use log::{error, info};

use stremio_service::shared::join_current_exe_dir;

pub struct Server {
    pub process: Option<Child>
}

impl Server {
    pub fn new() -> Self {
        Self {
            process: None
        }
    }

    pub fn start(&mut self) -> Result<(), Box<dyn Error>> {
        let node_binary_path = join_current_exe_dir("node");
        let ffmpeg_binary_path = join_current_exe_dir("ffmpeg");
        let server_location = join_current_exe_dir("server.js");

        let mut command = Command::new(node_binary_path);
        command.env("FFMPEG_BIN", ffmpeg_binary_path);
        command.arg(server_location);

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