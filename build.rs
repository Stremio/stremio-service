use std::{error::Error, fs, path::PathBuf, io::Cursor};

use tar::Archive;
use xz::bufread::XzDecoder;

const STREMIO_SERVER: &str = "https://dl.strem.io/four/master/server.js";
#[cfg(target_os = "windows")]
const NODE_WINDOWS_ARCHIVE: &str = "https://nodejs.org/dist/v18.12.1/node-v18.12.1-win-x64.zip";
#[cfg(target_os = "linux")]
const NODE_LINUX_ARCHIVE: &str = "https://nodejs.org/dist/v18.12.1/node-v18.12.1-linux-x64.tar.xz";
#[cfg(target_os = "macos")]
const NODE_MACOS_ARCHIVE: &str = "https://nodejs.org/dist/v18.12.1/node-v18.12.1-darwin-x64.tar.gz";

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

    #[cfg(target_os = "windows")] {
        extract_zip(NODE_WINDOWS_ARCHIVE, "node.exe", target_bin_path.clone())?;
    }

    #[cfg(target_os = "linux")] {
        extract_tar_xz(NODE_LINUX_ARCHIVE, "node", target_bin_path.clone())?;
    }

    #[cfg(target_os = "macos")] {
        extract_tar_xz(NODE_MACOS_ARCHIVE, "node", target_bin_path.clone())?;
    }

    let binaries_dir = current_dir.join("binaries");
    copy_binaries(binaries_dir, &target_bin_path)?;

    #[cfg(target_os = "windows")] {
        let resources_file = current_dir.join("resources").join("resources.rc");
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

fn extract_tar_xz(url: &str, file_name: &str, out: PathBuf) -> Result<(), Box<dyn Error>> {
    let target = out.join(file_name);
    if !target.exists() {
        let archive_file = reqwest::blocking::get(url)?.bytes()?;
        let decoded_stream = XzDecoder::new(Cursor::new(archive_file));
        let mut archive = Archive::new(decoded_stream);
        for entry in archive.entries()? {
            let mut file = entry?;
            let file_path = file.path()?;
            if file_path.is_file() && file_path.ends_with(file_name) {
                file.unpack_in(out.clone())?;
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
                    let final_file_name = file_name.replace(format!("-{}", platform_string).as_str(), "");
                    fs::copy(file.path(), path.join(final_file_name))?;
                }
            },
            _ => {}
        }
    }

    Ok(())
}