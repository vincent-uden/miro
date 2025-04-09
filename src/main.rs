// Widgets to use from iced-aw
// - Menu (File -> Open/Save and so on)

use std::{
    io,
    path::PathBuf,
    sync::{LazyLock, RwLock},
};

use app::{App, AppMessage};
use clap::Parser;
use iced::{Subscription, Theme};
use keymap::KeyMap;
use tracing_subscriber::EnvFilter;

mod app;
mod custom_serde_functions;
mod geometry;
mod keymap;
mod old_pdf;
mod pdf;
mod watch;

static APP_KEYMAP: LazyLock<RwLock<KeyMap<AppMessage>>> =
    LazyLock::new(|| KeyMap::<AppMessage>::default().into());

#[derive(Parser, Debug)]
#[command(version, name = "miro", about = "A pdf viewer")]
struct Args {
    #[arg(value_name = "PATH")]
    path: Option<PathBuf>,
}

fn main() -> iced::Result {
    tracing_subscriber::fmt()
        .with_writer(io::stdout)
        .with_env_filter(EnvFilter::new("miro"))
        .init();

    let args = Args::parse();

    iced::application("App", App::update, App::view)
        .antialiasing(true)
        .theme(theme)
        .font(iced_fonts::REQUIRED_FONT_BYTES)
        .subscription(App::subscription)
        .run_with(|| {
            let state = App::new();
            (state, match args.path {
                Some(p) => iced::Task::done(app::AppMessage::OpenFile(p)),
                None => iced::Task::none(),
            })
        })
}

pub fn theme(_: &App) -> Theme {
    Theme::Light
}
