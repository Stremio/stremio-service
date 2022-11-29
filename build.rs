use std::{error::Error, fs, path::PathBuf};

const STREMIO_SERVER: &str = "https://dl.strem.io/four/master/server.js";

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

    let binaries_dir = current_dir.join("binaries");
    copy_binaries(binaries_dir, &target_bin_path)?;

    #[cfg(target_os = "windows")] {
        let resources_file = current_dir.join("resources").join("resources.rc");
        embed_resource::compile(resources_file.to_str().unwrap());
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