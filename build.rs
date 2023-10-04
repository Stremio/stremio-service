use std::{env::consts::OS, error::Error, fs, path::PathBuf};

use once_cell::sync::Lazy;
use serde::Deserialize;
use url::Url;

#[cfg(target_os = "windows")]
use {
    chrono::{Datelike, Local},
    winres_edit::{resource_type, Id, Resources},
};

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

    if !SUPPORTED_OS.contains(&OS) {
        panic!(
            "OS {} not supported, supported OSes are: {:?}",
            OS, SUPPORTED_OS
        )
    }

    let current_dir = std::env::current_dir()?;
    let resources = current_dir.join("resources");
    let platform_bins = resources.join("bin").join(OS);

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
        let description =
            std::env::var("CARGO_PKG_DESCRIPTION").expect("Failed to read package description");

        let icon_path = resources.join("service.ico");
        let icon = icon_path.to_str().expect("Failed to find icon");

        let runtime_info = [
            ("ProductName", "Stremio Runtime"),
            ("FileDescription", &description),
            ("LegalCopyright", &copyright),
            ("CompanyName", "Stremio"),
            ("InternalName", "stremio-runtime"),
            ("OriginalFilename", "stremio-runtime.exe"),
        ];

        edit_exe_resources(
            &platform_bins.join("stremio-runtime.exe"),
            &resources.join("runtime.ico"),
            &runtime_info,
        )?;

        let mut res = winres::WindowsResource::new();
        res.set("FileDescription", &description);
        res.set("LegalCopyright", &copyright);
        res.set_icon_with_id(icon, "ICON");
        res.compile().unwrap();
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn edit_exe_resources(
    file_path: &PathBuf,
    icon_path: &PathBuf,
    info: &[(&str, &str)],
) -> Result<(), Box<dyn Error>> {
    let icon = std::fs::File::open(icon_path)?;
    let icon_dir = ico::IconDir::read(icon).unwrap();

    let mut resources = Resources::new(file_path);
    resources.load()?;
    resources.open()?;

    for (i, entry) in icon_dir.entries().iter().enumerate() {
        resources
            .find(resource_type::ICON, Id::Integer((i as u16) + 1))
            .expect(&format!("Failed to find icon {}", i))
            .replace(entry.data())?
            .update()?;
    }

    match resources.get_version_info() {
        Ok(version_info) => match version_info {
            Some(mut version_info) => {
                version_info.insert_strings(info).update()?;
            }
            _ => {}
        },
        Err(_) => eprintln!("Failed to get version info"),
    }

    resources.close();

    Ok(())
}
