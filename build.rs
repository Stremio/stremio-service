use std::{error::Error, fs, io::Cursor, path::Path};

use bytes::Bytes;

#[cfg(target_os = "windows")]
use {
    winres_edit::{Resources, resource_type, Id},
    std::path::PathBuf,
};

#[cfg(any(target_os = "linux", target_os = "macos"))]
use {
    tar::Archive,
    xz::bufread::XzDecoder,
    flate2::bufread::GzDecoder,
};

#[cfg(target_os = "windows")]
use chrono::{Datelike, Local};

const STREMIO_SERVER: &str = "https://dl.strem.io/four/master/server.js";
#[cfg(target_os = "windows")]
const NODE_WINDOWS_ARCHIVE: &str = "https://nodejs.org/dist/v18.12.1/node-v18.12.1-win-x64.zip";
#[cfg(target_os = "linux")]
const NODE_LINUX_ARCHIVE: &str = "https://nodejs.org/dist/v18.12.1/node-v18.12.1-linux-x64.tar.xz";
#[cfg(target_os = "macos")]
const NODE_MACOS_ARCHIVE: &str = "https://nodejs.org/dist/v18.12.1/node-v18.12.1-darwin-x64.tar.gz";

trait Decoder: std::io::Read {
    fn new(r: Cursor<Bytes>) -> Self;
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl Decoder for XzDecoder<Cursor<Bytes>> {
    fn new(r: Cursor<Bytes>) -> Self {
        XzDecoder::new(r)
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl Decoder for GzDecoder<Cursor<Bytes>> {
    fn new(r: Cursor<Bytes>) -> Self {
        GzDecoder::new(r)
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=src/");

    let current_dir = std::env::current_dir()?;
    let resource_dir = current_dir.join("resources");
    let resource_bin_dir = resource_dir.join("bin");

    let server_js_target = resource_bin_dir.join("server.js");
    if !server_js_target.exists() {
        let server_js_file = reqwest::blocking::get(STREMIO_SERVER)?.bytes()?;
        fs::write(resource_bin_dir.join("server.js"), server_js_file)?;
    }

    #[cfg(target_os = "windows")]
    {
        extract_zip(
            NODE_WINDOWS_ARCHIVE,
            "node.exe",
            "stremio-runtime.exe",
            &resource_bin_dir
        )?;

        let now = Local::now();
        let copyright = format!("Copyright © {} Smart Code OOD", now.year());
        let description = std::env::var("CARGO_PKG_DESCRIPTION").unwrap();

        let runtime_info = [
            ("ProductName", "Stremio Runtime"),
            ("FileDescription", &description),
            ("LegalCopyright", &copyright),
            ("CompanyName", "Stremio"),
            ("InternalName", "stremio-runtime"),
            ("OriginalFilename", "stremio-runtime.exe"),
        ];

        edit_exe_resources(
            &resource_bin_dir.join("stremio-runtime.exe"),
            &resource_dir.join("runtime.ico"),
            &runtime_info
        )?;

        let mut res = winres::WindowsResource::new();
        res.set_toolkit_path("C:\\Program Files (x86)\\Windows Kits\\10\\bin\\10.0.22621.0\\x64");
        res.set("FileDescription", &description);
        res.set("LegalCopyright", &copyright);
        res.set_icon_with_id("resources/service.ico", "ICON");
        res.compile().unwrap();
    }

    #[cfg(target_os = "linux")]
    {
        extract_tar::<XzDecoder<Cursor<Bytes>>>(
            NODE_LINUX_ARCHIVE,
            "bin/node",
            "stremio-runtime",
            &resource_bin_dir,
        )?;
    }

    #[cfg(target_os = "macos")]
    {
        extract_tar::<GzDecoder<Cursor<Bytes>>>(
            NODE_MACOS_ARCHIVE,
            "bin/node",
            "stremio-runtime",
            &resource_bin_dir,
        )?;
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn edit_exe_resources(file_path: &PathBuf, icon_path: &PathBuf, info: &[(&str, &str)]) -> Result<(), Box<dyn Error>> {
    let icon = std::fs::File::open(icon_path)?;
    let icon_dir = ico::IconDir::read(icon).unwrap();

    let mut resources = Resources::new(file_path);
    resources.load()?;
    resources.open()?;

    for (i, entry) in icon_dir.entries().iter().enumerate() {
        resources.find(resource_type::ICON, Id::Integer((i as u16) + 1))
            .expect(&format!("Failed to find icon {}", i))
            .replace(entry.data())?
            .update()?;
    }

    match resources.get_version_info() {
        Ok(version_info) => {
            match version_info {
                Some(mut version_info) => {
                    version_info
                        .insert_strings(info)
                        .update()?;
                },
                _ => {},
            }
        },
        Err(_) => eprintln!("Failed to get version info"),
    }

    resources.close();

    Ok(())
}

#[cfg(target_os = "windows")]
fn extract_zip(url: &str, file_path: &str, out_name: &str, out: &Path) -> Result<(), Box<dyn Error>> {
    let target = out.join(out_name);
    if !target.exists() {
        let tmp_dir = PathBuf::from(".tmp");
        fs::create_dir_all(tmp_dir.clone())?;

        let archive_file = reqwest::blocking::get(url)?.bytes()?;
        zip_extract::extract(Cursor::new(archive_file), &tmp_dir, true)?;
        fs::copy(tmp_dir.join(file_path), target)?;
        fs::remove_dir_all(tmp_dir)?;
    }

    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn extract_tar<D: Decoder>(
    url: &str,
    file_path: &str,
    out_name: &str,
    out: &Path,
) -> Result<(), Box<dyn Error>> {
    let target = out.join(out_name);
    if !target.exists() {
        let archive_file = reqwest::blocking::get(url)?.bytes()?;
        let decoded_stream = D::new(Cursor::new(archive_file));
        let mut archive = Archive::new(decoded_stream);
        for entry in archive.entries()? {
            let mut file = entry?;
            let path = file.path()?;
            if path.ends_with(file_path) {
                file.unpack(target.clone())?;
            }
        }
    }

    Ok(())
}
