use muda::{AcceleratorParseError, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use muda::accelerator::{Accelerator};
use crate::CONFIG;
use keybinds2::{Keybind};
use crate::config::BindableMessage;
use crate::app::AppMessage;
use crate::pdf::PdfMessage;
use std::fmt;
use std::str::FromStr;

pub struct AppMenu {
    menu: Menu,
    // These are special in the macos menu bar
    window_submenu: Submenu,
    help_submenu: Submenu,
}
// TODO:
// - Support different languages
// - test on non-mac OSes
impl AppMenu {
    pub fn new() -> Self {
        let menu = Menu::new();

        let app_submenu = Submenu::new("App", true);
        app_submenu
            .append_items(&[
                &PredefinedMenuItem::about(None, None),
                &PredefinedMenuItem::separator(),
                &PredefinedMenuItem::services(None),
                &PredefinedMenuItem::separator(),
                &PredefinedMenuItem::hide(None),
                &PredefinedMenuItem::hide_others(None),
                &PredefinedMenuItem::show_all(None),
                &PredefinedMenuItem::separator(),
                &PredefinedMenuItem::quit(None),
            ])
            .unwrap();
        // The first item appended always has to be the app (sub)menu (mac will override whatever else is present)
        menu.append(&app_submenu).unwrap();

        let cfg = CONFIG.read().unwrap();
        let file_submenu = Submenu::with_items(
            "&File",
            true,
            &[
                &MenuItem::with_id(
                    "OpenFileFinder",
                    "Open File",
                    true,
                    Some(
                        keybind_to_accelerator(
                            cfg.get_binding_for_msg(BindableMessage::OpenFileFinder)
                                .unwrap(),
                        )
                        .unwrap(),
                    ),
                ),
                &MenuItem::with_id(
                    "PrintPdf",
                    "Print",
                    true,
                    Some(
                        keybind_to_accelerator(
                            cfg.get_binding_for_msg(BindableMessage::PrintPdf).unwrap(),
                        )
                        .unwrap(),
                    ),
                ),
            ],
        )
        .unwrap();
        let view_submenu = Submenu::with_items(
            "&View",
            true,
            &[&MenuItem::with_id(
                "ToggleUIDarkMode",
                "Toggle UI Dark Mode",
                true,
                Some(
                    keybind_to_accelerator(
                        cfg.get_binding_for_msg(BindableMessage::ToggleDarkModeUi)
                            .unwrap(),
                    )
                    .unwrap(),
                ),
            )],
        )
        .unwrap();
        let window_submenu = Submenu::with_items(
            "&Window",
            true,
            &[
                &PredefinedMenuItem::minimize(None),
                &PredefinedMenuItem::maximize(None),
                &PredefinedMenuItem::close_window(None),
                &PredefinedMenuItem::fullscreen(None),
                &PredefinedMenuItem::bring_all_to_front(None),
            ],
        )
        .unwrap();
        let help_submenu = Submenu::new("&Help", true);
        menu.append_items(&[&file_submenu, &view_submenu, &window_submenu, &help_submenu])
            .unwrap();
        Self {
            menu,
            window_submenu,
            help_submenu,
        }
    }

    // This can not be called inside new(), must be done in the view.
    pub fn init(&self) {
        // #[cfg(target_os = "macos")]
        // {
        self.menu.init_for_nsapp();
        self.window_submenu.set_as_windows_menu_for_nsapp();
        self.help_submenu.set_as_help_menu_for_nsapp();
        // }
    }
}

// Dummy debug for now (muda doesn't implement debug for some reason?)
impl fmt::Debug for AppMenu {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AppMenu").finish()
    }
}

pub fn menu_bar_listener() -> impl iced::futures::Stream<Item = AppMessage> {
    iced::stream::channel(100, async |mut sender| {
        loop {
            if let Ok(event) = MenuEvent::receiver().try_recv() {
                match (&event.id().0).as_str() {
                    "OpenFileFinder" => {
                        let _ = sender.try_send(AppMessage::OpenNewFileFinder);
                    }
                    "PrintPdf" => {
                        let _ = sender.try_send(AppMessage::PdfMessage(PdfMessage::PrintPdf));
                    }
                    "ToggleUIDarkMode" => {
                        let _ = sender.try_send(AppMessage::ToggleDarkModeUi);
                    }
                    _ => {}
                }
            } else {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        }
    })
}

// Converts a keybinds2::Keybind into a muda::Accelerator.
// Ideally we'd want a single system for managing keybinds, but as muda uses Accelerator
// realistically I don't see how this is possible. This is fine for now.
pub fn keybind_to_accelerator(
    keybind: Keybind<BindableMessage>,
) -> Result<Accelerator, AcceleratorParseError> {
    let keybind_as_string = keybind.seq.as_slice()[0].to_string();
    return Accelerator::from_str(&keybind_as_string);
}
