// Copyright (C) 2017-2023 Smart code 203358507

use once_cell::sync::Lazy;
use url::Url;

/// Default Stremio web url
pub static DEV_STREMIO_URL: Lazy<Url> = Lazy::new(|| "https://localhost:8080".parse().unwrap());
/// Production Stremio web url
pub static STREMIO_URL: Lazy<Url> = Lazy::new(|| "https://web.stremio.com".parse().unwrap());
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
