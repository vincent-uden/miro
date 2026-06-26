use muda::accelerator::{KeyAccelerator};
use muda::AcceleratorParseError;
use crate::CONFIG;
use crate::common_menu::CommonMenuItem;
use keybinds2::{Keybind};
use crate::config::BindableMessage;
use crate::app::AppMessage;
use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;



pub struct Menu {
    menu: muda::Menu,
    recent_files_submenu: muda::Submenu,
    // These are special in the macos menu bar
    window_submenu: muda::Submenu,
    help_submenu: muda::Submenu,
}
impl Menu {
    pub fn new(recent_files: &[PathBuf]) -> Self {
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
        menu.append(&app_submenu).unwrap();

        let common_menu = crate::common_menu::CommonMenu::new();
        let skeleton = common_menu.skeleton;
        let recent_files_submenu = muda::Submenu::new("Recent Files", true);

        for tuple in skeleton {
            let submenu = muda::Submenu::new(format!("&{}", tuple.0), true);
            for common_menu_item in tuple.1 {
                match common_menu_item {
                    CommonMenuItem::Button(msg) => {
                        let label = msg.default_menu_label().unwrap_or("(unnamed)");
                        submenu.append(&new_menu_item(label, msg)).unwrap();
                    }
                    CommonMenuItem::RecentFiles => {
                        for path in recent_files {
                            let file_name = path
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_else(|| path.to_string_lossy().to_string());
                            let item = muda::MenuItem::with_id(
                                path.to_str().unwrap(),
                                file_name,
                                true,
                                None,
                            );
                            recent_files_submenu.append(&item).unwrap();
                        }
                        submenu.append(&recent_files_submenu).unwrap();
                    }
                    CommonMenuItem::Separator => {
                        submenu
                            .append(&muda::PredefinedMenuItem::separator())
                            .unwrap();
                    }
                }
            }
            menu.append(&submenu).unwrap();
        }

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
        menu.append_items(&[&window_submenu, &help_submenu])
            .unwrap();

        Self {
            menu,
            recent_files_submenu,
            window_submenu,
            help_submenu,
        }
    }

    pub fn update_recent_files(&self, recent_files: &[PathBuf]) {
        for _ in 0..(self.recent_files_submenu.items().len()) {
            self.recent_files_submenu.remove_at(0).unwrap();
        }
        for path in recent_files {
            let file_name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string_lossy().to_string());
            let item = muda::MenuItem::with_id(path.to_str().unwrap(), file_name, true, None);
            self.recent_files_submenu.append(&item).unwrap();
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

pub fn new_menu_item(label: &str, msg: AppMessage) -> muda::MenuItem {
    let cfg = CONFIG.read().unwrap();
    let menu_id = msg.menu_id().expect("Menu item must have a menu_id");
    let menu_item = muda::MenuItem::with_id(menu_id, label, true, None);
    if let Some(bindable) = msg.bindable() {
        if let Some(keybind) = cfg.get_binding_for_msg(bindable) {
            if let Ok(keyaccel) = keybind_to_keyaccelerator(keybind) {
                menu_item.set_key_accelerator(Some(keyaccel)).unwrap();
            }
        }
    }

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
                let id = (&event.id().0).as_str();
                match AppMessage::from_menu_id(id) {
                    Some(msg) => {
                        let _ = sender.try_send(msg);
                    }
                    None => {
                        let _ = sender.try_send(AppMessage::OpenFile(PathBuf::from(id)));
                    }
                }
            } else {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }
    })
}
