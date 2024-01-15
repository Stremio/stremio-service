// Copyright (C) 2017-2024 Smart Code OOD 203358507

use log::error;
use std::{
    env,
    path::{Path, PathBuf},
};
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
    system_tray::Icon::from_rgba(icon_rgba, icon_width, icon_height).expect("Failed to open icon")
}

pub fn get_current_exe_dir() -> PathBuf {
    let current_exe_location =
        env::current_exe().expect("Failed to get current executable location");
    let current_exe_dir = current_exe_location
        .parent()
        .expect("Failed to get current executable directory");

    PathBuf::from(current_exe_dir)
}

pub fn create_dir_if_does_not_exists(path: &Path) {
    if !path.exists() {
        if let Err(e) = std::fs::create_dir_all(path) {
            error!("Failed to create {:?} path: {}", path, e);
        }
    }
}
