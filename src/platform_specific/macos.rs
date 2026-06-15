use muda::accelerator::{KeyAccelerator};
use muda::AcceleratorParseError;
use crate::CONFIG;
use keybinds2::{Keybind};
use crate::config::BindableMessage;
use crate::app::AppMessage;
use std::fmt;
use std::str::FromStr;

pub struct Menu {
    menu: muda::Menu,
    // These are special in the macos menu bar
    window_submenu: muda::Submenu,
    help_submenu: muda::Submenu,
}
// TODO:
// - Support different languages
impl Menu {
    pub fn new() -> Self {
        let menu = muda::Menu::new();

        let app_submenu = muda::Submenu::new("App", true);
        app_submenu
            .append_items(&[
                &muda::PredefinedMenuItem::about(None, None),
                &muda::PredefinedMenuItem::separator(),
                &muda::PredefinedMenuItem::services(None),
                &muda::PredefinedMenuItem::separator(),
                &muda::PredefinedMenuItem::hide(None),
                &muda::PredefinedMenuItem::hide_others(None),
                &muda::PredefinedMenuItem::show_all(None),
                &muda::PredefinedMenuItem::separator(),
                &muda::PredefinedMenuItem::quit(None),
            ])
            .unwrap();

        let file_submenu = muda::Submenu::with_items(
            "&File",
            true,
            &[
                &new_menu_item("Open File", BindableMessage::OpenFileFinder),
                &new_menu_item("Print PDF", BindableMessage::PrintPdf),
            ],
        )
        .unwrap();
        let view_submenu = muda::Submenu::with_items(
            "&View",
            true,
            &[
                &new_menu_item("Toggle UI Dark Mode", BindableMessage::ToggleDarkModeUi),
                &new_menu_item("Toggle PDF Dark Mode", BindableMessage::ToggleDarkModePdf),
                &new_menu_item("Toggle Page Borders", BindableMessage::TogglePageBorders),
                &new_menu_item("Zoom In", BindableMessage::ZoomIn),
                &new_menu_item("Zoom Out", BindableMessage::ZoomOut),
                &new_menu_item("Zoom to 100%", BindableMessage::ZoomHome),
                &new_menu_item("Fit To Screen", BindableMessage::ZoomFit),
                &new_menu_item("Toggle Sidebar", BindableMessage::ToggleSidebar),
                &new_menu_item(
                    "Toggle Presentation Mode",
                    BindableMessage::TogglePresentationMode,
                ),
            ],
        )
        .unwrap();
        let layout_submenu = muda::Submenu::with_items(
            "&Layout",
            true,
            &[
                &new_menu_item("Toggle Sidebar", BindableMessage::ToggleSidebar),
                &new_menu_item("Single Page", BindableMessage::SinglePageLayout),
                &new_menu_item("Double Page", BindableMessage::DoublePageLayout),
                &new_menu_item(
                    "Double Page With Title Page",
                    BindableMessage::DoublePageTitlePageLayout,
                ),
                &new_menu_item("Presentation", BindableMessage::PresentationLayout),
            ],
        )
        .unwrap();
        let window_submenu = muda::Submenu::with_items(
            "&Window",
            true,
            &[
                &muda::PredefinedMenuItem::minimize(None),
                &muda::PredefinedMenuItem::maximize(None),
                &muda::PredefinedMenuItem::close_window(None),
                &muda::PredefinedMenuItem::fullscreen(None),
                &muda::PredefinedMenuItem::bring_all_to_front(None),
            ],
        )
        .unwrap();
        let help_submenu = muda::Submenu::new("&Help", true);

        // The first item appended always has to be the app (sub)menu (mac will override whatever else is present)
        menu.append_items(&[
            &app_submenu,
            &file_submenu,
            &view_submenu,
            &layout_submenu,
            &window_submenu,
            &help_submenu,
        ])
        .unwrap();
        Self {
            menu,
            window_submenu,
            help_submenu,
        }
    }

    pub fn init(&self) {
        #[cfg(target_os = "macos")]
        {
            self.menu.init_for_nsapp();
            self.window_submenu.set_as_windows_menu_for_nsapp();
            self.help_submenu.set_as_help_menu_for_nsapp();
        }
    }
}

// Our MenuItems will have id equal to the BindableMessage enum values as a string.
// For example, the Open file menu item will have id of "OpenFileFinder" (corresponding to BindableMessage::OpenFileFinder)
pub fn new_menu_item(label: &str, msg: BindableMessage) -> muda::MenuItem {
    let cfg = CONFIG.read().unwrap();
    let menu_item = muda::MenuItem::with_id(msg.to_string(), label, true, None);
    let keyaccel = keybind_to_keyaccelerator(cfg.get_binding_for_msg(msg).unwrap()).unwrap();
    menu_item.set_key_accelerator(Some(keyaccel)).unwrap();

    return menu_item;
}

// Converts a keybinds2::Keybind into a muda::KeyAccelerator.
// Ideally we'd want a single system for managing keybinds, but as muda uses KeyAccelerator
// realistically I don't see how this is possible. This is fine for now.
pub fn keybind_to_keyaccelerator(
    keybind: Keybind<BindableMessage>,
) -> Result<KeyAccelerator, AcceleratorParseError> {
    let keybind_as_string = keybind.seq.as_slice()[0].to_string();
    return KeyAccelerator::from_str(&keybind_as_string);
}

// Dummy debug for now (muda doesn't implement debug for some reason?)
impl fmt::Debug for Menu {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AppMenu").finish()
    }
}

pub fn menu_listener() -> impl iced::futures::Stream<Item = AppMessage> {
    iced::stream::channel(100, async |mut sender| {
        loop {
            if let Ok(event) = muda::MenuEvent::receiver().try_recv() {
                let msg = BindableMessage::from_str((&event.id().0).as_str()).unwrap();
                let _ = sender.try_send(AppMessage::from(msg));
            } else {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }
    })
}
