use std::{error::Error, fs, path::PathBuf, io::Cursor};

use bytes::Bytes;
use flate2::bufread::GzDecoder;
use tar::Archive;
use xz::bufread::XzDecoder;

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
    let resource_dir = current_dir.join("resources");
    let resource_bin_dir = resource_dir.join("bin");


    let server_js_target = resource_bin_dir.join("server.js");
    if !server_js_target.exists() {
        let server_js_file = reqwest::blocking::get(STREMIO_SERVER)?.bytes()?;
        fs::write(resource_bin_dir.join("server.js"), server_js_file)?;
    }

    #[cfg(target_os = "windows")] {
        extract_zip(NODE_WINDOWS_ARCHIVE, "node.exe", resource_bin_dir.clone())?;
    }

    #[cfg(target_os = "linux")] {
        extract_tar::<XzDecoder<Cursor<Bytes>>>(NODE_LINUX_ARCHIVE, "bin/node", "node", &resource_bin_dir)?;
    }

    #[cfg(target_os = "macos")] {
        extract_tar::<GzDecoder<Cursor<Bytes>>>(NODE_MACOS_ARCHIVE, "bin/node", "node", &resource_bin_dir)?;
    }

    #[cfg(target_os = "windows")] {
        let resources_file = resource_dir.join("resources.rc");
        embed_resource::compile(resources_file.to_str().unwrap());
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

fn extract_tar<D: Decoder + std::io::Read>(url: &str, file_path: &str, out_name: &str, out: &PathBuf) -> Result<(), Box<dyn Error>> {
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