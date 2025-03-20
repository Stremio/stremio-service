// Copyright (C) 2017-2025 Smart Code OOD 203358507

#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]
use std::error::Error;

use anyhow::Context;
use clap::Parser;
use env_logger::Env;

use stremio_service::app::{handle_stremio_protocol, Application, Config};
use stremio_service::args::Args;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let cli = Args::parse();

    if let Some(url) = cli.open.as_ref() {
        if !url.is_empty() {
            handle_stremio_protocol(url.clone());
        }
    }

    let home_dir = dirs::home_dir().context("Failed to get home dir")?;
    let cache_dir = dirs::cache_dir().context("Failed to get cache dir")?;

    // use current exe directory
    #[cfg(not(debug_assertions))]
    let service_bins_dir = stremio_service::util::get_current_exe_dir();

    // use the `resources/bin/{linux|windows|macos}` directory
    #[cfg(debug_assertions)]
    let service_bins_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("bin")
        .join(std::env::consts::OS);

    let config = Config::new(cli, home_dir, cache_dir, service_bins_dir)?;
    log::info!("Using service configuration: {:#?}", config);

    let application = Application::new(config);

    Ok(application.run().await?)
}
