use anyhow::{anyhow, Context};
use tao::{
    event_loop::EventLoop,
    menu::{ContextMenu, CustomMenuItem, MenuItemAttributes},
    system_tray::{SystemTray, SystemTrayBuilder},
    TrayId,
};

use crate::util::load_icon;

use super::{Icons, ServerStatus, TrayStatus};

pub struct TrayMenu {
    pub system_tray: SystemTray,
    /// Open stremio web menu element
    pub open: CustomMenuItem,
    /// Quit service element
    pub quit: CustomMenuItem,
    /// the server status menu item
    pub server: CustomMenuItem,
}

impl TrayMenu {
    pub fn new(event_loop: &EventLoop<()>) -> anyhow::Result<TrayMenu, anyhow::Error> {
        let (tray_menu, open, quit, server_status) = TrayMenu::create_menu(TrayStatus::default());

        let icon_file = Icons::get("icon.png").ok_or_else(|| anyhow!("Failed to get icon file"))?;
        let icon = load_icon(icon_file.data.as_ref());

        let system_tray = SystemTrayBuilder::new(icon, Some(tray_menu))
            .with_id(TrayId::new("main"))
            .build(event_loop)
            .context("Failed to build the application system tray")?;

        Ok(TrayMenu {
            system_tray,
            open,
            quit,
            server: server_status,
        })
    }

    pub fn create_menu(
        status: impl Into<Option<TrayStatus>>,
    ) -> (ContextMenu, CustomMenuItem, CustomMenuItem, CustomMenuItem) {
        let mut tray_menu = ContextMenu::new();
        let open_item = tray_menu.add_item(MenuItemAttributes::new("Open Stremio Web"));
        let quit_item = tray_menu.add_item(MenuItemAttributes::new("Quit"));

        let version_item_label = format!("Service v{}", env!("CARGO_PKG_VERSION"));
        let version_item = MenuItemAttributes::new(version_item_label.as_str()).with_enabled(false);
        tray_menu.add_item(version_item);

        let status: Option<TrayStatus> = status.into();
        let server_status = match status.unwrap_or_default().server_js {
            ServerStatus::NotRunning => format!("Server is not running"),
            ServerStatus::Running {
                version,
                ..
            } => {
                format!("Server {version} is running")
            }
        };

        let server_item =
            tray_menu.add_item(MenuItemAttributes::new(&server_status).with_enabled(false));

        (tray_menu, open_item, quit_item, server_item)
    }

    pub fn set_status(&mut self, status: TrayStatus) {
        let (tray_menu, open, quit, server_status) = TrayMenu::create_menu(status);

        self.system_tray.set_menu(&tray_menu);
        self.open = open;
        self.quit = quit;
        self.server = server_status;
    }
}
