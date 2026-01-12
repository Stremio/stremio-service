// Copyright (C) 2017-2026 Smart Code OOD 203358507

use std::path::PathBuf;

use anyhow::{Context, Error};
use rand::Rng as _;
use url::Url;

#[cfg(feature = "bundled")]
use crate::util;
use crate::{args::Args, constants::UPDATE_ENDPOINT, server};

#[derive(Debug, Clone)]
pub struct Config {
    /// The Home directory of the user running the service
    /// used to make the application an autostart one (on `*nix` systems)
    #[cfg_attr(any(not(feature = "bundled"), target_os = "windows"), allow(dead_code))]
    pub home_dir: PathBuf,

    pub tray_icon: PathBuf,

    /// The lockfile that guards against running multiple instances of the service.
    pub lockfile: PathBuf,

    /// The server configuration
    pub server: server::Config,
    pub updater_endpoint: Url,
    pub skip_update: bool,
    pub force_update: bool,
}

impl Config {
    /// Try to create by validating the application configuration.
    ///
    /// It will initialize the server [`server::Config`] and if it fails it will return an error.
    ///
    /// If `self_update` is `true` and it is a supported platform for the updater (see [`IS_UPDATER_SUPPORTED`])
    /// it will check for the existence of the `updater` binary at the given location.
    pub fn new(args: Args) -> Result<Self, Error> {
        let home_dir = dirs::home_dir().context("Failed to get home dir")?;
        let cache_dir = dirs::cache_dir().context("Failed to get cache dir")?;

        let tray_icon = if cfg!(target_os = "linux") {
            let runtime_dir = dirs::runtime_dir().context("Failed to get runtime dir")?;
            runtime_dir.join("stremio-service")
        } else {
            PathBuf::new()
        };

        let lockfile = cache_dir.join("lock");

        #[cfg(feature = "bundled")]
        // use the installed dir if we've built the app with `bundled` feature.
        let service_bins_dir = util::get_current_exe_dir();
        #[cfg(not(feature = "bundled"))]
        // use the `resources/bin/{linux|windows|macos}` directory
        let service_bins_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("bin")
            .join(std::env::consts::OS);

        let server =
            server::Config::new(service_bins_dir).context("Server configuration failed")?;

        let updater_endpoint = if let Some(endpoint) = args.updater_endpoint {
            endpoint
        } else {
            let mut url = Url::parse(Self::get_random_updater_endpoint().as_str())?;
            if args.release_candidate {
                url.query_pairs_mut().append_pair("rc", "true");
            }
            url
        };

        Ok(Self {
            home_dir,
            tray_icon,
            lockfile,
            server,
            updater_endpoint,
            skip_update: args.skip_updater,
            force_update: args.force_update,
        })
    }

    fn get_random_updater_endpoint() -> String {
        let mut rng = rand::thread_rng();
        let index = rng.gen_range(0..UPDATE_ENDPOINT.len());
        UPDATE_ENDPOINT[index].to_string()
    }
}
