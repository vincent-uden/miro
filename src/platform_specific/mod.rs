use crate::{AppMessage};
use iced::Subscription;
use iced::Task;

pub mod iced_aw;
pub mod macos;

pub fn startup_tasks() -> Vec<Task<AppMessage>> {
    vec![
        #[cfg(target_os = "macos")]
        Task::done(AppMessage::InitializeMacMenu),
    ]
}

pub fn listeners() -> Vec<Subscription<AppMessage>> {
    vec![
        #[cfg(target_os = "macos")]
        Subscription::run(macos::menu_listener),
    ]
}
