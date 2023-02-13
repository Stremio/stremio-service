use std::{env, error::Error, fs, io::Cursor, path::PathBuf};

use bytes::Bytes;
use flate2::bufread::GzDecoder;
#[cfg(not(target_os = "windows"))]
use tar::Archive;
use xz::bufread::XzDecoder;

#[cfg(target_os = "windows")]
use chrono::{Datelike, Local};

#[cfg(target_os = "windows")]
extern crate winres;

const STREMIO_SERVER: &str = "https://dl.strem.io/four/master/server.js";
#[cfg(target_os = "windows")]
const NODE_WINDOWS_ARCHIVE: &str = "https://nodejs.org/dist/v18.12.1/node-v18.12.1-win-x64.zip";
#[cfg(target_os = "linux")]
const NODE_LINUX_ARCHIVE: &str = "https://nodejs.org/dist/v18.12.1/node-v18.12.1-linux-x64.tar.xz";
#[cfg(target_os = "macos")]
const NODE_MACOS_ARCHIVE: &str = "https://nodejs.org/dist/v18.12.1/node-v18.12.1-darwin-x64.tar.gz";

trait Decoder {
    fn new(r: Cursor<Bytes>) -> Self;
}

impl Decoder for XzDecoder<Cursor<Bytes>> {
    fn new(r: Cursor<Bytes>) -> Self {
        XzDecoder::new(r)
    }
}

impl Decoder for GzDecoder<Cursor<Bytes>> {
    fn new(r: Cursor<Bytes>) -> Self {
        GzDecoder::new(r)
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=src/");

    let current_dir = std::env::current_dir()?;
    let target_dir = current_dir.join("target");

    let target_bin_path = if cfg!(debug_assertions) {
        target_dir.join("debug")
    } else {
        target_dir.join("release")
    };

    let server_js_target = target_bin_path.join("server.js");
    if !server_js_target.exists() {
        let server_js_file = reqwest::blocking::get(STREMIO_SERVER)?.bytes()?;
        fs::write(target_bin_path.join("server.js"), server_js_file)?;
    }

    #[cfg(target_os = "windows")]
    {
        extract_zip(NODE_WINDOWS_ARCHIVE, "node.exe", target_bin_path.clone())?;
    }

    #[cfg(target_os = "linux")]
    {
        extract_tar::<XzDecoder<Cursor<Bytes>>>(
            NODE_LINUX_ARCHIVE,
            "bin/node",
            "node",
            &target_bin_path,
        )?;
    }

    #[cfg(target_os = "macos")]
    {
        extract_tar::<GzDecoder<Cursor<Bytes>>>(
            NODE_MACOS_ARCHIVE,
            "bin/node",
            "node",
            &target_bin_path,
        )?;
    }

    let binaries_dir = current_dir.join("binaries");
    copy_binaries(binaries_dir, &target_bin_path)?;

    #[cfg(target_os = "windows")]
    {
        let now = Local::now();
        let copyright = format!("Copyright © {} Smart Code OOD", now.year());
        let mut res = winres::WindowsResource::new();
        res.set(
            "FileDescription",
            &env::var("CARGO_PKG_DESCRIPTION").unwrap(),
        );
        res.set("LegalCopyright", &copyright);
        res.set_icon_with_id("resources/service.ico", "ICON");
        res.compile().unwrap();
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn extract_zip(url: &str, file_name: &str, out: PathBuf) -> Result<(), Box<dyn Error>> {
    let target = out.join(file_name);
    if !target.exists() {
        let tmp_dir = PathBuf::from(".tmp");
        fs::create_dir_all(tmp_dir.clone())?;

        let archive_file = reqwest::blocking::get(url)?.bytes()?;
        zip_extract::extract(Cursor::new(archive_file), &tmp_dir, true)?;
        fs::copy(tmp_dir.join(file_name), target)?;
        fs::remove_dir_all(tmp_dir)?;
    }

    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn extract_tar<D: Decoder + std::io::Read>(
    url: &str,
    file_path: &str,
    out_name: &str,
    out: &PathBuf,
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

fn copy_binaries(binaries_dir: PathBuf, path: &PathBuf) -> Result<(), Box<dyn Error>> {
    let platform_string = std::env::consts::OS;

    for entry in fs::read_dir(binaries_dir)? {
        match entry {
            Ok(file) => {
                let file_name = file.file_name().into_string().unwrap();
                if file_name.contains(&platform_string) {
                    let final_file_name =
                        file_name.replace(format!("-{}", platform_string).as_str(), "");
                    fs::copy(file.path(), path.join(final_file_name))?;
                }
            }
            _ => {}
        }
    }

    Ok(())
}
