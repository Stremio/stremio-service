use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Whether or not to skip the updater
    ///
    /// This options is not used for `*nix` systems
    #[arg(short, long)]
    pub skip_updater: bool,

    /// If set, the updater will skip version check
    ///
    /// This options is not used for `*nix` systems
    #[arg(short, long)]
    pub force_update: bool,

    /// The endpoint to use for the updater
    ///
    /// Overrides the default endpoint
    #[clap(short, long)]
    pub updater_endpoint: Option<String>,

    /// Open an URL with a custom `stremio://` scheme.
    ///
    /// If empty URL or no url is provided, the service will skip this argument.
    #[clap(short, long)]
    pub open: Option<String>,
}