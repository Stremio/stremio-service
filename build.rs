use std::{error::Error, fs, io::Write, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let current_dir = std::env::current_dir()?;

    let version_file_path = current_dir.join("resources").join("version");
    create_version_file(version_file_path)?;

    let out_dir = current_dir.join("target");
    let binaries_dir = current_dir.join("binaries");
    copy_binaries(binaries_dir, out_dir)?;

    Ok(())
}

fn create_version_file(path: PathBuf) -> Result<(), Box<dyn Error>> {
    let version = env!("CARGO_PKG_VERSION");
    let mut file = fs::File::create(path)?;
    file.write(version.as_bytes())?;

    Ok(())
}

fn copy_binaries(binaries_dir: PathBuf, out_dir: PathBuf) -> Result<(), Box<dyn Error>> {
    let platform_string = std::env::consts::OS;

    let debug_dir = out_dir.join("debug");
    let release_dir = out_dir.join("release");

    for entry in fs::read_dir(binaries_dir)? {
        match entry {
            Ok(file) => {
                let file_name = file.file_name().into_string().unwrap();
                if file_name.contains(&platform_string) {
                    let final_file_name = file_name.replace(format!("-{}", platform_string).as_str(), "");
                    fs::copy(file.path(), debug_dir.join(final_file_name.clone()))?;
                    fs::copy(file.path(), release_dir.join(final_file_name))?;
                }
            },
            _ => {}
        }
    }

    Ok(())
}