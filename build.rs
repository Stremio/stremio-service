use std::{error::Error, fs::File, io::Write};

fn main() -> Result<(), Box<dyn Error>> {
    let version = env!("CARGO_PKG_VERSION");

    let current_dir = std::env::current_dir()?;
    let version_file = current_dir.join("resources").join("version");

    let mut file = File::create(version_file)?;
    file.write(version.as_bytes())?;

    Ok(())
}