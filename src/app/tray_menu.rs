// Copyright (C) 2017-2023 Smart code 203358507

use anyhow::{anyhow, Context};
use log::trace;
use once_cell::sync::Lazy;
use tao::{
    event_loop::EventLoop,
    menu::{ContextMenu, CustomMenuItem, MenuId, MenuItemAttributes},
    system_tray::{SystemTray, SystemTrayBuilder},
    TrayId,
};

use crate::util::load_icon;

use super::{Icons, ServerTrayStatus, TrayStatus};

pub struct TrayMenu {
    tray_status: TrayStatus,
    pub system_tray: SystemTray,
    /// Open stremio web menu element
    pub open: CustomMenuItem,
    /// Quit service element
    pub quit: CustomMenuItem,
    /// the server status menu item
    pub server: Option<CustomMenuItem>,
    /// Restart the server
    pub restart: CustomMenuItem,
    /// Explicitly start the server
    pub start: Option<CustomMenuItem>,
    /// Explicitly stop the server
    pub stop: Option<CustomMenuItem>,
}

pub static MAIN_ID: Lazy<TrayId> = Lazy::new(|| TrayId::new("main"));
pub static OPEN_MENU: Lazy<MenuId> = Lazy::new(|| MenuId::new("open"));
pub static QUIT_MENU: Lazy<MenuId> = Lazy::new(|| MenuId::new("quit"));
pub static STOP_SERVER_MENU: Lazy<MenuId> = Lazy::new(|| MenuId::new("stop server"));
pub static START_SERVER_MENU: Lazy<MenuId> = Lazy::new(|| MenuId::new("start server"));
pub static RESTART_SERVER_MENU: Lazy<MenuId> = Lazy::new(|| MenuId::new("restart server"));

/// User server action from the Tray menu
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerAction {
    Start,
    Stop,
    /// First stops the server child process then it starts it back up.
    Restart,
}

impl ServerAction {
    /// Get the [`MenuId`] of the given action in the [`TrayMenu`].
    pub fn menu_id(&self) -> MenuId {
        match self {
            ServerAction::Start => *START_SERVER_MENU,
            ServerAction::Stop => *STOP_SERVER_MENU,
            ServerAction::Restart => *RESTART_SERVER_MENU,
        }
    }
}
pub enum MenuEvent {
    UpdateTray(TrayStatus),
}

impl TrayMenu {
    pub fn new(event_loop: &EventLoop<MenuEvent>) -> anyhow::Result<TrayMenu> {
        TrayMenu::with_menu(event_loop, TrayStatus::default())
    }

    pub fn with_menu(
        event_loop: &EventLoop<MenuEvent>,
        status: TrayStatus,
    ) -> anyhow::Result<Self> {
        let (tray_menu, server, open, quit, restart, start, stop) =
            Self::create_menu(status.clone());
        let icon_file = Icons::get("icon.png").ok_or_else(|| anyhow!("Failed to get icon file"))?;
        let icon = load_icon(icon_file.data.as_ref());

        let system_tray = SystemTrayBuilder::new(icon, Some(tray_menu))
            .with_id(*MAIN_ID)
            .build(event_loop)
            .context("Failed to build the application system tray")?;

        Ok(Self {
            tray_status: status,
            system_tray,
            open,
            quit,
            server,
            restart,
            start,
            stop,
        })
    }

    /// Use when action on the server is taken (like restarting)
    pub fn unset_server(&mut self) {
        let mut tray_status = self.tray_status.clone();
        tray_status.server_js = None;

        self.set_status(tray_status)
    }

    pub fn set_status(&mut self, status: TrayStatus) {
        let (tray_menu, server_status, open, quit, restart, start, stop) =
            Self::create_menu(status.clone());
        trace!("Set system tray menu status: {status:#?}");

        self.system_tray.set_menu(&tray_menu);
        self.open = open;
        self.quit = quit;
        self.server = server_status;
        self.restart = restart;
        self.start = start;
        self.stop = stop;
    }

    fn create_menu(
        status: impl Into<Option<TrayStatus>>,
    ) -> (
        ContextMenu,
        Option<CustomMenuItem>,
        CustomMenuItem,
        CustomMenuItem,
        CustomMenuItem,
        Option<CustomMenuItem>,
        Option<CustomMenuItem>,
    ) {
        let status: Option<TrayStatus> = status.into();

        let server_tray_status = status.unwrap_or_default().server_js;
        let has_server_status = server_tray_status.is_some();
        let server_running = matches!(server_tray_status, Some(ServerTrayStatus::Running { .. }));
        let server_restarting = matches!(server_tray_status, Some(ServerTrayStatus::Restarting));
        let server_stopped = matches!(server_tray_status, Some(ServerTrayStatus::Stopped));

        let mut tray_menu = ContextMenu::new();
        let open_item =
            tray_menu.add_item(MenuItemAttributes::new("Open Stremio Web").with_id(*OPEN_MENU));

        let restart_server_item = tray_menu.add_item(
            MenuItemAttributes::new("Restart Server")
                .with_enabled(has_server_status && !server_restarting)
                .with_id(*RESTART_SERVER_MENU),
        );

        #[cfg(debug_assertions)]
        let stop_server_item = Some(
            tray_menu.add_item(
                MenuItemAttributes::new("Stop Server")
                    .with_enabled(has_server_status && server_running)
                    .with_id(*STOP_SERVER_MENU),
            ),
        );
        #[cfg(not(debug_assertions))]
        let stop_server_item = None;

        #[cfg(not(debug_assertions))]
        let start_server_item = None;
        #[cfg(debug_assertions)]
        let start_server_item = Some(
            tray_menu.add_item(
                MenuItemAttributes::new("Start Server")
                    .with_enabled(has_server_status && server_stopped)
                    .with_id(*START_SERVER_MENU),
            ),
        );

        let quit_item = tray_menu.add_item(MenuItemAttributes::new("Quit").with_id(*QUIT_MENU));

        let version_item_label = format!("Service v{}", env!("CARGO_PKG_VERSION"));
        let version_item = MenuItemAttributes::new(version_item_label.as_str()).with_enabled(false);
        tray_menu.add_item(version_item);

        #[cfg(not(debug_assertions))]
        let debug = String::new();
        #[cfg(debug_assertions)]
        let debug = format!(
            "\nUpdated every: {}s",
            crate::Application::update_every().as_secs()
        );

        let server_item = server_tray_status.map(|server_tray_status| {
            let server_status = match server_tray_status {
                ServerTrayStatus::Stopped => format!("Server is not running{debug}"),
                ServerTrayStatus::Restarting => format!("Server is restarting{debug}"),
                ServerTrayStatus::Running { info } => {
                    format!("Server v{} is running{debug}", info.version)
                }
            };

            tray_menu.add_item(MenuItemAttributes::new(&server_status).with_enabled(false))
        });

        (
            tray_menu,
            server_item,
            open_item,
            quit_item,
            restart_server_item,
            start_server_item,
            stop_server_item,
        )
    }
}
