use std::{error::Error, path::PathBuf};

use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
struct MacOSMetadata {
    name: String,
    display_name: String,
    identifier: String,
    icon: Vec<String>,
    executable: String,
    bins: Vec<Vec<String>>,
}

#[derive(Clone, Debug, Deserialize)]
struct Metadata {
    macos: Option<MacOSMetadata>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let manifest_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let manifest = cargo_toml::Manifest::<Metadata>::from_path_with_metadata(manifest_path)?;

    let metadata = manifest
        .package
        .expect("Failed to parse package")
        .metadata
        .expect("Failed to parse manifest.package.metadata")
        .macos
        .expect("Failed to parse manifest.package.metadata.macos");

    let target_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("macos");
    std::fs::create_dir_all(target_path.clone())?;

    let bundle_path = target_path
        .join(metadata.name + ".app");
    std::fs::create_dir_all(bundle_path.clone())?;

    let contents_path = bundle_path
        .join("Contents");
    std::fs::create_dir_all(contents_path.clone())?;

    let info_plist = format!("
        <?xml version=\"1.0\" encoding=\"UTF-8\"?>
        <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">
        <plist version=\"1.0\">
            <dict>
                <key>CFBundleDisplayName</key>
                <string>{}</string>
                <key>CFBundleIdentifier</key>
                <string>{}</string>
                <key>CFBundleVersion</key>
                <string>{}</string>
                <key>CFBundleShortVersionString</key>
                <string>{}</string>
                <key>CFBundleIconFile</key>
                <string>{}</string>
                <key>CFBundleExecutable</key>
                <string>{}</string>
            </dict>
        </plist>
    ", metadata.display_name, metadata.identifier, env!("CARGO_PKG_VERSION"), env!("CARGO_PKG_VERSION"), metadata.icon[1], metadata.executable);
    std::fs::write(contents_path.join("Info.plist"), info_plist)?;

    let bins_path = contents_path
        .join("MacOS");
    std::fs::create_dir_all(bins_path.clone())?;
        
    for bin in metadata.bins {
        let target_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(bin[0].clone());
        std::fs::copy(target_path, bins_path.join(bin[1].clone()))?;
    }

    let resources_path = contents_path
        .join("Resources");
    std::fs::create_dir_all(resources_path.clone())?;
        
    let icon_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(metadata.icon[0].clone());
    std::fs::copy(icon_path, resources_path.join(metadata.icon[1].clone()))?;

    Ok(())
}