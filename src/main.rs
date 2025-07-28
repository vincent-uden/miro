use std::{
    fs, io,
    path::PathBuf,
    sync::{LazyLock, RwLock},
};

use app::App;
use bookmarks::BookmarkStore;
use clap::Parser;
use config::Config;
use iced::{Theme, window::icon::from_file_data};
use tracing::info;
use tracing_subscriber::EnvFilter;

mod app;
mod bookmarks;
mod config;
mod geometry;
mod icons;
mod pdf;
mod rpc;
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

    match home::home_dir() {
        Some(path) => fs::create_dir_all(path.join(".config/miro-pdf"))
            .expect("Couldn't create the required config directory"),
        None => eprintln!("Couldn't find home directory"),
    }

    if let Ok(cfg) = Config::system_config() {
        let mut config = CONFIG.write().unwrap();
        *config = cfg;
        info!(
            "Using system config file located at {}",
            Config::system_config_path()
                .expect(
                    "Managed to load a config file without being able to determine its location"
                )
                .canonicalize()
                .unwrap()
                .to_str()
                .unwrap()
        );
    }

    iced::application("App", App::update, App::view)
        .antialiasing(true)
        .theme(theme)
        .font(iced_fonts::REQUIRED_FONT_BYTES)
        .subscription(App::subscription)
        .window(settings())
        .run_with(move || {
            let state = App::new(BookmarkStore::system_store().unwrap_or_default());
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

//#[cfg(target_os = "windows")]
pub fn settings() -> iced::window::Settings {
    use iced::window::Settings;

    let icon_img = include_bytes!("../assets/logo.png");
    let icon = from_file_data(icon_img, None).ok();

    Settings {
        icon,
        ..Default::default()
    }
}
