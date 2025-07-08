use std::{
    io,
    path::PathBuf,
    sync::{LazyLock, RwLock},
};

use app::App;
use bookmarks::BookmarkStore;
use clap::Parser;
use config::Config;
use iced::{Theme, window::icon::from_file_data};
use once_cell::sync::OnceCell;
use pdf::cache::{WorkerCommand, WorkerResponse, worker_main};
use tokio::sync::{Mutex, mpsc};
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
static WORKER_RX: OnceCell<Mutex<tokio::sync::mpsc::UnboundedReceiver<WorkerResponse>>> =
    OnceCell::new();
static RENDER_GENERATION: OnceCell<Mutex<usize>> = OnceCell::new();

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

    let (command_tx, command_rx) = mpsc::unbounded_channel::<WorkerCommand>();
    let (result_tx, result_rx) = mpsc::unbounded_channel::<WorkerResponse>();

    let _worker_handle = std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(worker_main(command_rx, result_tx));
    });

    WORKER_RX.get_or_init(move || Mutex::new(result_rx));
    RENDER_GENERATION.get_or_init(|| Mutex::new(0));

    match Config::system_config() {
        Ok(cfg) => {
            let mut config = CONFIG.write().unwrap();
            *config = cfg;
            info!(
                "Using system config file located at {}",
                Config::system_config_path()
                    .expect(
                        "Managed to load a config file without being able to determine its location"
                    )
                    .to_str()
                    .unwrap()
            );
        }
        Err(_) => {}
    }

    iced::application("App", App::update, App::view)
        .antialiasing(true)
        .theme(theme)
        .font(iced_fonts::REQUIRED_FONT_BYTES)
        .subscription(App::subscription)
        .window(settings())
        .run_with(move || {
            let state = App::new(
                command_tx,
                BookmarkStore::system_store().unwrap_or_default(),
            );
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
    use iced::window::Settings;

    let icon_img = include_bytes!("../assets/logo.png");
    let icon = from_file_data(icon_img, None).ok();

    Settings {
        icon,
        ..Default::default()
    }
}
