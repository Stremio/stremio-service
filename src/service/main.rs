mod updater;
mod server;

use std::{error::Error, path::PathBuf, sync::Once, collections::HashMap};
use fruitbasket::{FruitCallbackKey, FruitObjcCallback, kAEGetURL, kInternetEventClass};
use fslock::LockFile;
use log::{error, info};
use clap::{Parser};
use objc::{runtime::{Object, Class, Sel}, msg_send, sel, sel_impl, declare::ClassDecl, Message};
use objc_foundation::{NSObject, INSObject};
use objc_id::{Shared, Id, WeakId};
use tao::{event_loop::{EventLoop, ControlFlow}, menu::{ContextMenu, MenuItemAttributes, MenuId}, system_tray::{SystemTrayBuilder, SystemTray}, TrayId, event::Event};
#[cfg(not(target_os = "linux"))]
use native_dialog::{MessageDialog, MessageType};
use rust_embed::RustEmbed;

#[cfg(not(target_os = "linux"))]
use updater::{fetch_update, run_updater};
use server::Server;
use stremio_service::{
    config::{DATA_DIR, STREMIO_URL, DESKTOP_FILE_PATH, DESKTOP_FILE_NAME, AUTOSTART_CONFIG_PATH, LAUNCH_AGENTS_PATH, APP_IDENTIFIER, APP_NAME},
    shared::{load_icon, create_dir_if_does_not_exists}
};
use urlencoding::encode;

#[derive(RustEmbed)]
#[folder = "icons"]
struct Icons;

#[derive(Parser, Debug)]
pub struct Options {
    #[clap(short, long)]
    pub skip_updater: bool,
    #[clap(short, long)]
    pub open: Option<String>,
}

struct ObjcWrapper<'a> {
    objc: Id<ObjcSubclass, Shared>,
    map: HashMap<FruitCallbackKey, FruitObjcCallback<'a>>,
}

impl<'a> ObjcWrapper<'a> {
    fn take(&mut self) -> Id<ObjcSubclass, Shared> {
        let weak = WeakId::new(&self.objc);
        weak.load().unwrap()
    }
}

enum ObjcSubclass {}

unsafe impl Message for ObjcSubclass { }

static OBJC_SUBCLASS_REGISTER_CLASS: Once = Once::new();

impl ObjcSubclass {
    /// Call a registered Rust callback
    fn dispatch_cb(wrap_ptr: u64, key: FruitCallbackKey, obj: *mut Object) {
        if wrap_ptr == 0 {
            return;
        }
        let objcwrap: &mut ObjcWrapper = unsafe { &mut *(wrap_ptr as *mut ObjcWrapper) };
        if let Some(ref cb) = objcwrap.map.get(&key) {
            cb(obj);
        }
    }
}

/// Define an ObjC class and register it with the ObjC runtime
impl INSObject for ObjcSubclass {
    fn class() -> &'static Class {
        OBJC_SUBCLASS_REGISTER_CLASS.call_once(|| {
            let superclass = NSObject::class();
            let mut decl = ClassDecl::new("ObjcSubclass", superclass).unwrap();
            decl.add_ivar::<u64>("_rustwrapper");
            
            /// Callback for events from Apple's NSAppleEventManager
            extern fn objc_apple_event(this: &Object, _cmd: Sel, event: u64, _reply: u64) {
                let ptr: u64 = unsafe { *this.get_ivar("_rustwrapper") };
                ObjcSubclass::dispatch_cb(ptr,
                                          FruitCallbackKey::Method("handleEvent:withReplyEvent:"),
                                          event as *mut Object);
            }
            /// NSApplication delegate callback
            extern fn objc_did_finish(this: &Object, _cmd: Sel, event: u64) {
                let ptr: u64 = unsafe { *this.get_ivar("_rustwrapper") };
                ObjcSubclass::dispatch_cb(ptr,
                                          FruitCallbackKey::Method("applicationDidFinishLaunching:"),
                                          event as *mut Object);
            }
            /// NSApplication delegate callback
            extern fn objc_will_finish(this: &Object, _cmd: Sel, event: u64) {
                let ptr: u64 = unsafe { *this.get_ivar("_rustwrapper") };
                ObjcSubclass::dispatch_cb(ptr,
                                          FruitCallbackKey::Method("applicationWillFinishLaunching:"),
                                          event as *mut Object);
            }
            /// NSApplication delegate callback
            extern "C" fn objc_open_file(
                this: &Object,
                _cmd: Sel,
                _application: u64,
                file: u64,
            ) -> bool {
                let ptr: u64 = unsafe { *this.get_ivar("_rustwrapper") };
                ObjcSubclass::dispatch_cb(
                    ptr,
                    FruitCallbackKey::Method("application:openFile:"),
                    file as *mut Object,
                );

                true
            }
            /// Register the Rust ObjcWrapper instance that wraps this object
            ///
            /// In order for an instance of this ObjC owned object to reach back
            /// into "pure Rust", it needs to know the location of Rust
            /// functions.  This is accomplished by wrapping it in a Rust struct,
            /// which is itself in a Box on the heap to ensure a fixed location
            /// in memory.  The address of this wrapping struct is given to this
            /// object by casting the Box into a raw pointer, and then casting
            /// that into a u64, which is stored here.
            extern fn objc_set_rust_wrapper(this: &mut Object, _cmd: Sel, ptr: u64) {
                unsafe {this.set_ivar("_rustwrapper", ptr);}
            }

            unsafe {
                // Register all of the above handlers as true ObjC selectors:
                let f: extern fn(&mut Object, Sel, u64) = objc_set_rust_wrapper;
                decl.add_method(sel!(setRustWrapper:), f);
                let f: extern fn(&Object, Sel, u64, u64) = objc_apple_event;
                decl.add_method(sel!(handleEvent:withReplyEvent:), f);
                let f: extern fn(&Object, Sel, u64) = objc_did_finish;
                decl.add_method(sel!(applicationDidFinishLaunching:), f);
                let f: extern fn(&Object, Sel, u64) = objc_will_finish;
                decl.add_method(sel!(applicationWillFinishLaunching:), f);
                let f: extern "C" fn(&Object, Sel, u64, u64) -> bool = objc_open_file;
                decl.add_method(sel!(application:openFile:), f);
            }

            decl.register();
        });

        Class::get("ObjcSubclass").unwrap()
    }
}

static FINISHED_LAUNCHING: Once = Once::new();

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let options = Options::parse();

    if let Some(open_url) = options.open {
        if open_url.starts_with("stremio://") {
            let url = open_url.replace("stremio://", "https://");
            open_stremio_web(Some(url));
        }
    }

    let home_dir = dirs::home_dir()
        .expect("Failed to get home dir");
    let data_location = home_dir.join(DATA_DIR);

    std::fs::create_dir_all(data_location.clone())?;

    let lock_path = data_location.join("lock");
    let mut lockfile = LockFile::open(&lock_path)?;

    if !lockfile.try_lock()? {
        info!("Exiting, another instance is running.");
        return Ok(())
    }

    make_it_autostart(home_dir);

    #[cfg(not(target_os = "linux"))]
    if !options.skip_updater {
        let current_version = env!("CARGO_PKG_VERSION");
        info!("Fetching updates for v{}", current_version);

        match fetch_update(&current_version).await {
            Ok(response) => {
                match response {
                    Some(update) => {
                        info!("Found update v{}", update.version.to_string());

                        let title = "Stremio Service";
                        let message = format!("Update v{} is available.\nDo you want to update now?", update.version.to_string());
                        let do_update = MessageDialog::new()
                            .set_type(MessageType::Info)
                            .set_title(title)
                            .set_text(&message)
                            .show_confirm()
                            .unwrap();

                        if do_update {
                            run_updater(update.file.browser_download_url);
                            return Ok(());
                        }
                    },
                    None => {}
                }
            },
            Err(e) => error!("Failed to fetch updates: {}", e)
        }
    }

    let mut server = Server::new();
    server.start()?;

    let event_loop = EventLoop::new();

    let (mut system_tray, open_item_id, quit_item_id) = create_system_tray(&event_loop)?;

    event_loop.run(move |event, _event_loop, control_flow| {
        *control_flow = ControlFlow::Wait;

        FINISHED_LAUNCHING.call_once(|| {
            unsafe {
                let objc = ObjcSubclass::new().share();
                let mut rustobjc = Box::new(ObjcWrapper {
                    objc,
                    map: HashMap::new(),
                });
                let ptr: u64 = &*rustobjc as *const ObjcWrapper as u64;
                let _:() = msg_send![rustobjc.objc, setRustWrapper: ptr];

                rustobjc.map.insert(FruitCallbackKey::Method("handleEvent:withReplyEvent:"), Box::new(move |event| {
                    let url: String = fruitbasket::parse_url_event(event);
                    info!("Received URL: {}", url);
                    MessageDialog::new()
                        .set_type(MessageType::Info)
                        .set_text(&url)
                        .show_confirm()
                        .unwrap();
                }));

                let cls = Class::get("NSAppleEventManager").unwrap();
                let manager: *mut Object = msg_send![cls, sharedAppleEventManager];
                let objc = (*rustobjc).take();
                let _:() = msg_send![
                    manager,
                    setEventHandler: objc
                    andSelector: sel!(handleEvent:withReplyEvent:)
                    forEventClass: kInternetEventClass
                    andEventID: kAEGetURL
                ];
            }
        });

        match event {
            Event::MenuEvent {
                menu_id,
                ..
            } => {
                if menu_id == open_item_id {
                    open_stremio_web(None);
                }
                if menu_id == quit_item_id {
                    system_tray.take();
                    *control_flow = ControlFlow::Exit;
                }
            },
            Event::LoopDestroyed => {
                server.stop();
            },
            _ => (),
        }
    });
}

fn make_it_autostart(home_dir: PathBuf) {
    #[cfg(target_os = "linux")] {
        create_dir_if_does_not_exists(AUTOSTART_CONFIG_PATH);

        let from = PathBuf::from(DESKTOP_FILE_PATH).join(DESKTOP_FILE_NAME);
        let to = PathBuf::from(home_dir).join(AUTOSTART_CONFIG_PATH).join(DESKTOP_FILE_NAME);

        if !to.exists() {
            if let Err(e) = std::fs::copy(from, to) {
                error!("Failed to copy desktop file to autostart location: {}", e);
            }
        }
    }

    #[cfg(target_os = "macos")] {
        let plist_launch_agent = format!("
            <?xml version=\"1.0\" encoding=\"UTF-8\"?>
            <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">
            <plist version=\"1.0\">
            <dict>  
                <key>Label</key>
                <string>{}</string>
                <key>ProgramArguments</key>
                <array>
                    <string>/usr/bin/open</string>
                    <string>-a</string>
                    <string>{}</string>
                </array>
                <key>RunAtLoad</key>
                <true/>
            </dict>
            </plist>
        ", APP_IDENTIFIER, APP_NAME);

        let launch_agents_path = PathBuf::from(LAUNCH_AGENTS_PATH);
        create_dir_if_does_not_exists(
            launch_agents_path.to_str()
                .expect("Failed to convert PathBuf to str")
        );

        let plist_path = launch_agents_path.join(format!("{}.plist", APP_IDENTIFIER));
        if !plist_path.exists() {
            if let Err(e) = std::fs::write(plist_path, plist_launch_agent.as_bytes()) {
                error!("Failed to create a plist file in LaunchAgents dir: {}", e);
            }
        }
    }
}

fn create_system_tray(event_loop: &EventLoop<()>) -> Result<(Option<SystemTray>, MenuId, MenuId), Box<dyn Error>> {
    let mut tray_menu = ContextMenu::new();
    let open_item = tray_menu.add_item(MenuItemAttributes::new("Open Stremio Web"));
    let quit_item = tray_menu.add_item(MenuItemAttributes::new("Quit"));

    let version_item_label = format!("v{}", env!("CARGO_PKG_VERSION"));
    let version_item = MenuItemAttributes::new(version_item_label.as_str())
        .with_enabled(false);
    tray_menu.add_item(version_item);

    let icon_file = Icons::get("icon.png")
        .expect("Failed to get icon file");
    let icon = load_icon(icon_file.data.as_ref());

    let system_tray = SystemTrayBuilder::new(icon.clone(), Some(tray_menu))
        .with_id(TrayId::new("main"))
        .build(event_loop)
        .unwrap();

    Ok((
        Some(system_tray),
        open_item.id(),
        quit_item.id()
    ))
}

fn open_stremio_web(addon_manifest_url: Option<String>) {
    let mut url = STREMIO_URL.to_string();
    if let Some(p) = addon_manifest_url {
        url = format!("{}/#/addons?addon={}", STREMIO_URL, &encode(&p));
    }

    match open::that(url) {
        Ok(_) => info!("Opened Stremio Web in the browser"),
        Err(e) => error!("Failed to open Stremio Web: {}", e)
    }
}