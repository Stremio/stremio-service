#![cfg_attr(
    all(target_os = "windows", not(debug_assertions),),
    windows_subsystem = "windows"
)]
use anyhow::Context;
use clap::Parser;
use std::error::Error;

use stremio_service::app::{handle_stremio_protocol, Application, Config};
use stremio_service::args::Args;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let cli = Args::parse();

    if let Some(url) = cli.open.as_ref() {
        if !url.is_empty() {
            handle_stremio_protocol(url.clone());
        }
    }

    let home_dir = dirs::home_dir().context("Failed to get home dir")?;

    #[cfg(feature = "bundled")]
    // use the installed dir if we've built the app with `bundled` feature.
    let service_bins_dir = stremio_service::util::get_current_exe_dir();
    #[cfg(not(feature = "bundled"))]
    // use the `resources/bin` directory
    let service_bins_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("bin");

    let config = Config::new(cli, home_dir, service_bins_dir)?;
    log::info!("Using service configuration: {:?}", config);

    let application = Application::new(config);

    Ok(application.run().await?)
}
