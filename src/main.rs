// Widgets to use from iced-aw
// - Menu (File -> Open/Save and so on)

use std::{
    io,
    path::PathBuf,
    sync::{LazyLock, RwLock},
};

use app::App;
use clap::Parser;
use iced::Theme;
use keymap::Config;
use tracing_subscriber::EnvFilter;

mod app;
mod geometry;
mod keymap;
mod pdf;
mod watch;

const DARK_THEME: Theme = Theme::TokyoNight;
const LIGHT_THEME: Theme = Theme::Light;

static CONFIG: LazyLock<RwLock<Config>> = LazyLock::new(|| RwLock::new(Config::default()));

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
            (
                state,
                match args.path {
                    Some(p) => iced::Task::done(app::AppMessage::OpenFile(p)),
                    None => iced::Task::none(),
                },
            )
        })
}

pub fn theme(app: &App) -> Theme {
    match app.dark_mode {
        true => DARK_THEME,
        false => LIGHT_THEME,
    }
}
