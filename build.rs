use std::{env::consts::OS, error::Error, fs, path::PathBuf};

use once_cell::sync::Lazy;

use serde::Deserialize;
use url::Url;

#[cfg(target_os = "windows")]
use chrono::{Datelike, Local};

static STREMIO_SERVER_URL: Lazy<Url> = Lazy::new(|| "https://dl.strem.io/server/".parse().unwrap());

#[derive(Clone, Debug, Deserialize)]
struct ServerMetadata {
    /// The server.js version to be fetched from `dl.strem.io`.
    ///
    /// It can be semantic versioning or other
    version: String,
}

/// Cargo.toml metadata which we're interested in
#[derive(Clone, Debug, Deserialize)]
struct Metadata {
    server: ServerMetadata,
}

const SUPPORTED_OS: &[&str] = &["linux", "macos", "windows"];

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=src/");
    // Rebuild the application and download a new server.js if the version file has changed.
    for supported_os in SUPPORTED_OS.iter() {
        println!("cargo:rerun-if-changed=resources/bin/{supported_os}/server_version.txt",);
    }

    if !SUPPORTED_OS.contains(&OS) {
        panic!(
            "OS {} not supported, supported OSes are: {:?}",
            OS, SUPPORTED_OS
        )
    }

    let current_dir = std::env::current_dir()?;
    let platform_bins = current_dir.join("resources").join("bin").join(OS);

    #[cfg(not(feature = "offline-build"))]
    {
        let server_js_target = platform_bins.join("server.js");
        // keeps track of the server.js version in order to update it if versions mismatch
        let server_js_version_file = platform_bins.join("server_version.txt");

        let manifest_version = {
            let manifest_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
            let manifest = cargo_toml::Manifest::<Metadata>::from_path_with_metadata(manifest_path)
                .expect("Cannot read the manifest metadata");

            let server_metadata = manifest
                .package
                .expect("Failed to parse package")
                .metadata
                .expect("Failed to parse manifest.package.metadata")
                .server;

            server_metadata.version
        };

        let download_server_js = || -> Result<(), Box<dyn Error>> {
            let version_url = STREMIO_SERVER_URL
                .clone()
                .join(&format!("{manifest_version}/desktop/server.js"))
                .expect("Should never fail");

            let server_js_file = reqwest::blocking::get(version_url)?
                .error_for_status()?
                .bytes()?;

            fs::write(&server_js_target, server_js_file)?;
            // replace content in the version file
            fs::write(&server_js_version_file, &manifest_version)?;
            Ok(())
        };

        match (
            server_js_target.exists(),
            fs::read_to_string(&server_js_version_file).ok(),
        ) {
            // if server.js does not exist (no matter if the version file exist)
            // or if the server.js file exist but we don't have a version file.
            (false, _) | (true, None) => download_server_js()?,
            (true, Some(version)) => {
                if manifest_version != version {
                    download_server_js()?
                }
                // else do nothing, we have the same version
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        let now = Local::now();
        let copyright = format!("Copyright © {} Smart Code OOD", now.year());
        let mut res = winres::WindowsResource::new();
        res.set(
            "FileDescription",
            &std::env::var("CARGO_PKG_DESCRIPTION").unwrap(),
        );
        res.set("LegalCopyright", &copyright);
        res.set_icon_with_id("resources/service.ico", "ICON");
        res.compile().unwrap();
    }

    Ok(())
}
