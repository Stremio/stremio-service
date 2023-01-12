use anyhow::Context;
use clap::Parser;
use std::error::Error;

use stremio_service::app::{handle_stremio_protocol, Application, Config};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Whether or not to skip the updater
    ///
    /// This options is not used for `*nix` systems
    #[arg(short, long)]
    pub skip_updater: bool,

    /// Open an URL with a custom `stremio://` scheme.
    ///
    /// If empty URL or no url is provided, the service will skip this argument.
    #[clap(short, long)]
    pub open: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let cli = Cli::parse();

    match cli.open {
        Some(url) if url.is_empty() => {
            handle_stremio_protocol(url);
        }
        _ => {}
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

    let config = Config::new(home_dir, service_bins_dir, !cli.skip_updater)?;
    log::info!("Using service configuration: {:?}", config);

    let application = Application::new(config);

    Ok(application.run().await?)
}
