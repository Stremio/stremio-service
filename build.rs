use std::{error::Error, fs, io::Write, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let current_dir = std::env::current_dir()?;
    let out_dir = current_dir.join("target");

    let target_bin_path = if cfg!(debug_assertions) {
        out_dir.join("debug")
    } else {
        out_dir.join("release")
    };

    create_version_file(&target_bin_path)?;

    let binaries_dir = current_dir.join("binaries");
    copy_binaries(binaries_dir, &target_bin_path)?;

    if cfg!(target_os = "windows") {
        let resources_file = current_dir.join("resources").join("resources.rc");
        embed_resource::compile(resources_file.to_str().unwrap());
    }

    Ok(())
}

fn create_version_file(path: &PathBuf) -> Result<(), Box<dyn Error>> {
    let version_file_path = path.join("version");
    let mut file = fs::File::create(version_file_path)?;

    let version = env!("CARGO_PKG_VERSION");
    file.write(version.as_bytes())?;

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