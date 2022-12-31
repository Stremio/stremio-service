use anyhow::Context;
use clap::{Parser, Subcommand};
use std::error::Error;

use stremio_service::{
    app::{handle_stremio_protocol, Application, Config},
    server,
};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    #[arg(short, long)]
    /// Whether or not to skip the updater
    ///
    /// This options is not used for `*nix` systems
    pub skip_updater: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Open an URL with a custom `stremio://` scheme.
    ///
    /// Used when installing addons or opening the web UI.
    Open {
        #[arg(short, long)]
        url: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let cli = Cli::parse();

    if let Some(Command::Open { url }) = cli.command {
        handle_stremio_protocol(url);
    }

    let home_dir = dirs::home_dir().context("Failed to get home dir")?;

    #[cfg(feature = "bundled")]
    // use the installed dir if we've built the app with `bundled` feature.
    let server_bins_dir = stremio_service::util::get_current_exe_dir();
    #[cfg(not(feature = "bundled"))]
    // use the `resources/bin` directory
    let server_bins_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("bin");

    let server_config =
        server::Config::at_dir(server_bins_dir).context("Server.js configuration failed")?;
    let config = Config::new(home_dir, server_config, !cli.skip_updater);
    log::info!("Using service configuration: {:?}", config);

    let application = Application::new(config);

    Ok(application.run().await?)
}
