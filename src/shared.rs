use std::{env, path::PathBuf};
use tao::system_tray;

pub fn load_icon(buffer: &[u8]) -> system_tray::Icon {
    let (icon_rgba, icon_width, icon_height) = {
        let image = image::load_from_memory(buffer)
            .expect("Failed to open icon path")
            .into_rgba8();
        let (width, height) = image.dimensions();
        let rgba = image.into_raw();

        (rgba, width, height)
    };
    system_tray::Icon::from_rgba(icon_rgba, icon_width, icon_height)
        .expect("Failed to open icon")
}

pub fn get_current_exe_dir() -> PathBuf {
    let current_exe_location = env::current_exe()
        .expect("Failed to get current executable location");
    let current_exe_dir = current_exe_location.parent()
        .expect("Failed to get current executable directory");

    PathBuf::from(current_exe_dir)
}

pub fn join_current_exe_dir(append: &str) -> PathBuf {
    let current_exe_dir = get_current_exe_dir();
    current_exe_dir.join(PathBuf::from(append))
}

pub fn get_version_string() -> String {
    let version_file_path = join_current_exe_dir("version");

    std::fs::read_to_string(version_file_path)
        .expect("Failed to read version file")
}