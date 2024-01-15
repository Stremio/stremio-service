// Copyright (C) 2017-2024 Smart Code OOD 203358507

use std::path::PathBuf;

#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::os::unix::fs::PermissionsExt;

use anyhow::Context;
use env_logger::Env;
use log::info;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
struct MacOSMetadata {
    name: String,
    display_name: String,
    identifier: String,
    icon: Vec<String>,
    copyright: String,
    url_scheme: String,
    executable: String,
    bins: Vec<Vec<String>>,
}

#[derive(Clone, Debug, Deserialize)]
struct Metadata {
    macos: Option<MacOSMetadata>,
}

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let manifest_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let manifest = cargo_toml::Manifest::<Metadata>::from_path_with_metadata(manifest_path)
        .context("Cannot read the manifest metadata")?;

    let metadata = manifest
        .package
        .context("Failed to parse package")?
        .metadata
        .context("Failed to parse manifest.package.metadata")?
        .macos
        .context("Failed to parse manifest.package.metadata.macos")?;

    let target_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("macos");
    std::fs::create_dir_all(target_path.clone()).context(format!(
        "Failed to create target dir: {}",
        target_path.display()
    ))?;

    let bundle_path = target_path.join(metadata.name + ".app");

    if bundle_path.exists() {
        std::fs::remove_dir_all(bundle_path.clone()).context("Failed to remove old bundle")?;
    }
    std::fs::create_dir_all(bundle_path.clone())?;

    let contents_path = bundle_path.join("Contents");
    std::fs::create_dir_all(contents_path.clone()).context(format!(
        "Failed to create directory: {}",
        contents_path.display()
    ))?;

    let info_plist = format!("
        <?xml version=\"1.0\" encoding=\"UTF-8\"?>
        <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">
        <plist version=\"1.0\">
            <dict>
                <key>CFBundleDisplayName</key>
                <string>{display_name}</string>
                <key>CFBundleIdentifier</key>
                <string>{identifier}</string>
                <key>CFBundleVersion</key>
                <string>{version}</string>
                <key>CFBundleShortVersionString</key>
                <string>{version}</string>
                <key>CFBundleIconFile</key>
                <string>{icon_file}</string>
                <key>CFBundleExecutable</key>
                <string>{executable}</string>
                <key>NSHumanReadableCopyright</key>
                <string>{copyright}</string>
                <key>CFBundleURLTypes</key>
                <array>
                    <dict>
                        <key>CFBundleURLName</key>
                        <string>{url_name}</string>
                        <key>CFBundleURLSchemes</key>
                        <array>
                            <string>{url_scheme}</string>
                        </array>
                    </dict>
                </array>
            </dict>
        </plist>
    ",
        display_name = metadata.display_name,
        identifier = metadata.identifier,
        version = env!("CARGO_PKG_VERSION"),
        icon_file = metadata.icon[1],
        executable = metadata.executable,
        copyright = metadata.copyright,
        url_name = metadata.display_name,
        url_scheme = metadata.url_scheme
    );
    std::fs::write(contents_path.join("Info.plist"), info_plist).context(format!(
        "Failed to write Info.plist to {}",
        contents_path.join("Info.plist").to_str().unwrap()
    ))?;

    let bins_path = contents_path.join("MacOS");
    std::fs::create_dir_all(bins_path.clone()).context(format!(
        "Failed to create directory: {}",
        bins_path.to_str().unwrap()
    ))?;

    for bin in metadata.bins {
        info!("Copying {} to {}", bin[0], bin[1]);
        let target_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(bin[0].clone());
        std::fs::copy(target_path, bins_path.join(bin[1].clone()))
            .context(format!("Failed to copy {} to {}", bin[0], bin[1]))?;
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        // Make the file executable
        std::fs::set_permissions(
            bins_path.join(bin[1].clone()),
            std::fs::Permissions::from_mode(0o755),
        )
        .context(format!("Failed to set permissions for {}", bin[1]))?;
    }
    info!("All files copied");

    let resources_path = contents_path.join("Resources");
    std::fs::create_dir_all(resources_path.clone()).context(format!(
        "Failed to create directory: {}",
        resources_path.to_str().unwrap()
    ))?;

    let icon_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(metadata.icon[0].clone());
    std::fs::copy(icon_path, resources_path.join(metadata.icon[1].clone())).unwrap_or_else(|_| {
        panic!(
            "Failed to copy {} to {}",
            metadata.icon[0], metadata.icon[1]
        )
    });
    info!("Finished bundling Macos .app");

    Ok(())
}
