// Copyright (C) 2017-2026 Smart Code OOD 203358507

#![cfg_attr(
    all(target_os = "windows", feature = "bundled"),
    windows_subsystem = "windows"
)]
use std::error::Error;

use clap::Parser;
use env_logger::Env;

use stremio_service::app::{handle_stremio_protocol, Application};
use stremio_service::args::Args;
use stremio_service::config::Config;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let args = Args::parse();

    if let Some(url) = args.open.as_ref() {
        if !url.is_empty() {
            handle_stremio_protocol(url.clone());
        }
    }

    let config = Config::new(args)?;
    log::info!("Using service configuration: {:#?}", config);

    let application = Application::new(config);

    Ok(application.run().await?)
}
