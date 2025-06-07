// Widgets to use from iced-aw
// - Menu (File -> Open/Save and so on)

use std::{
    io,
    path::PathBuf,
    sync::{LazyLock, RwLock, mpsc},
};

use anyhow::anyhow;
use app::App;
use clap::Parser;
use iced::{
    Theme,
    window::{Icon, icon::from_file_data},
};
use keymap::Config;
use pdf::cache::{WorkerCommand, WorkerResponse, worker_main};
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

    let (command_tx, command_rx) = mpsc::channel::<WorkerCommand>();
    let (result_tx, mut result_rx) = mpsc::channel::<WorkerResponse>();

    let _worker_handle = std::thread::spawn(move || {
        worker_main(command_rx, result_tx);
    });

    iced::application("App", App::update, App::view)
        .antialiasing(true)
        .theme(theme)
        .font(iced_fonts::REQUIRED_FONT_BYTES)
        .subscription(App::subscription)
        .window(settings())
        .run_with(move || {
            let state = App::new(command_tx, result_rx);
            (state, match args.path {
                Some(p) => iced::Task::done(app::AppMessage::OpenFile(p)),
                None => iced::Task::none(),
            })
        })
}

pub fn theme(app: &App) -> Theme {
    match app.dark_mode {
        true => DARK_THEME,
        false => LIGHT_THEME,
    }
}

//#[cfg(target_os = "windows")]
pub fn settings() -> iced::window::Settings {
    use iced::window::{self, Settings};

    let icon_img = include_bytes!("../assets/logo.png");
    let icon = from_file_data(icon_img, None).ok();

    Settings {
        icon,
        ..Default::default()
    }
}
