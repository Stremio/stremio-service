use clap::Parser;
use log::{error, info};
use std::{error::Error, io::Cursor, path::PathBuf, process::Command};

use stremio_service::util::get_current_exe_dir;

#[derive(Parser, Debug)]
pub struct Options {
    #[clap(short, long)]
    pub url: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let options = Options::parse();

    if options.url.len() > 0 {
        info!("Downloading {}...", options.url);
        let archive = reqwest::get(options.url).await?.bytes().await?;

        let current_exe_dir = get_current_exe_dir();

        info!("Extracting archive to {:?}...", current_exe_dir);
        let extracted = zip_extract::extract(Cursor::new(archive), &current_exe_dir, true);

        match extracted {
            Ok(_) => info!("Successfully extracted archive."),
            Err(e) => error!("Failed to extract archive: {}", e),
        }
    }

    run_service();

    Ok(())
}

fn run_service() {
    let current_exe_dir = get_current_exe_dir();
    let updater_binary_path = current_exe_dir.join(PathBuf::from("service"));

    let mut command = Command::new(updater_binary_path);
    command.arg("--skip-updater");

    match command.spawn() {
        Ok(process) => {
            let process_pid = process.id();
            info!("Stremio Service started. (PID {:?})", process_pid);
        }
        Err(err) => error!("Stremio Service couldn't be started: {err}"),
    }
}
