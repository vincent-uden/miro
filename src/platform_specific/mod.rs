use crate::{AppMessage};
use iced::Subscription;
use iced::Task;

pub mod macos;

pub fn startup_tasks() -> Vec<Task<AppMessage>> {
    let mut tasks = Vec::new();
    #[cfg(target_os = "macos")]
    {
        tasks.push(Task::done(AppMessage::InitializeMacMenu));
    }
    return tasks;
}

pub fn listeners() -> Vec<Subscription<AppMessage>> {
    let mut listeners = Vec::new();
    #[cfg(target_os = "macos")]
    {
        listeners.push(Subscription::run(macos::menu_listener));
    }
    return listeners;
}
