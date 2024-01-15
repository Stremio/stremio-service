// Copyright (C) 2017-2024 Smart Code OOD 203358507

pub const STREMIO_URL: &str = "https://web.stremio.com";
pub const APP_IDENTIFIER: &str = "com.stremio.service";
pub const APP_NAME: &str = "StremioService";

pub const DESKTOP_FILE_PATH: &str = "/usr/share/applications";
pub const DESKTOP_FILE_NAME: &str = "com.stremio.service.desktop";
pub const AUTOSTART_CONFIG_PATH: &str = ".config/autostart";
pub const LAUNCH_AGENTS_PATH: &str = "Library/LaunchAgents";

pub const UPDATE_ENDPOINT: [&str; 3] = [
    "https://www.strem.io/updater/check?product=stremio-service",
    "https://www.stremio.com/updater/check?product=stremio-service",
    "https://www.stremio.net/updater/check?product=stremio-service",
];
