#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    fs,
    io::{self, IsTerminal, Read},
    path::PathBuf,
    sync::{LazyLock, RwLock},
    time::SystemTime,
};

use anyhow::anyhow;
use app::App;
use bookmarks::BookmarkStore;
use clap::Parser;
use recent_files::RecentFiles;
use config::Config;
use iced::{window::icon::from_file_data, Color, Font, Theme};
use tracing::info;
use tracing_subscriber::EnvFilter;

use crate::app::AppMessage;

mod app;
mod bookmarks;
mod common_menu;
mod config;
mod geometry;
mod icons;
mod jumplist;
mod pdf;
mod platform_specific;
mod recent_files;
mod rpc;
mod watch;

// of the screen
// TODO: Figure out why hovering over a menu disables all other inputs/buttons in the program (write
// my own menu??)

const DARK_THEME: Theme = Theme::TokyoNight;

static CONFIG: LazyLock<RwLock<Config>> = LazyLock::new(|| RwLock::new(Config::default()));

struct TempFile(PathBuf);

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[derive(Parser, Debug)]
#[command(version, name = "miro", about = "A pdf viewer")]
struct Args {
    #[arg(value_name = "PATH")]
    path: Option<PathBuf>,
    #[arg(
        short,
        long,
        help = "Launch the program in fullscreen mode (can be combined with --presentation)"
    )]
    fullscreen: bool,
    #[arg(
        short,
        long,
        help = "Launch the program in presentation mode (can be combined with --fullscreen)"
    )]
    presentation: bool,
    #[arg(
        long,
        value_name = "URL",
        help = "Download a pdf from the specified URL to a temporary file and open it"
    )]
    url: Option<String>,
}

fn bytes_to_tmp(bytes: &[u8], file_prefix: &str) -> anyhow::Result<PathBuf> {
    let ts = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let tmp = std::env::temp_dir().join(format!("miro-{file_prefix}-{ts}.pdf"));
    match fs::write(&tmp, bytes) {
        Ok(_) => Ok(tmp),
        Err(e) => Err(anyhow!("{e}")),
    }
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(io::stdout)
        .with_env_filter(EnvFilter::new("miro"))
        .init();

    let mut args = Args::parse();

    // NOTE: Used to automatically delete the file when exiting the program (normally or when
    // crashing)
    let mut tmp_file = None;
    if !io::stdin().is_terminal() && args.path.is_none() && args.url.is_none() {
        let mut bytes = Vec::new();
        match io::stdin().read_to_end(&mut bytes) {
            Ok(_) => match bytes_to_tmp(&bytes, "stdin") {
                Ok(tmp) => {
                    args.path = Some(tmp.clone());
                    tmp_file = Some(TempFile(tmp.clone()));
                }
                Err(e) => {
                    eprintln!("Failed to write to temporary file: {e}");
                }
            },
            Err(e) => {
                eprintln!("Failed to read from stdin: {e}");
            }
        }
    }

    if let Some(url) = args.url {
        let resp = reqwest::blocking::get(&url).map_err(|e| anyhow!("{e}"))?;
        let bytes: Vec<_> = resp.bytes()?.into_iter().collect();
        match bytes_to_tmp(&bytes, "url") {
            Ok(tmp) => {
                args.path = Some(tmp.clone());
                tmp_file = Some(TempFile(tmp.clone()));
            }
            Err(e) => {
                eprintln!("Faield to write to temporary file {e}");
            }
        }
    }

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
    let cfg_fullscreen;
    let cfg_presentation;
    {
        let config = CONFIG.read().unwrap();
        cfg_presentation = config.open_presentation_default;
        cfg_fullscreen = config.open_fullscreen_default;
    }
    Ok(iced::application(
        move || {
            let path = args.path.clone();
            let state = App::new(
                BookmarkStore::system_store().unwrap_or_default(),
                RecentFiles::system_store().unwrap_or_default(),
            );
            let startup_tasks = match path {
                Some(p) if tmp_file.is_some() => iced::Task::done(app::AppMessage::OpenTempFile(p)),
                Some(p) => iced::Task::done(app::AppMessage::OpenFile(p)),
                None => iced::Task::none(),
            };
            let mut startup_tasks =
                startup_tasks.chain(iced::window::latest().map(app::AppMessage::FoundWindowId));

            for task in platform_specific::startup_tasks().into_iter() {
                startup_tasks = startup_tasks.chain(task);
            }

            // NOTE: The default state is in windowed, non presentation mode. Using the toggles is
            // thus deterministic.
            if args.fullscreen || cfg_fullscreen {
                startup_tasks = startup_tasks.chain(iced::Task::done(AppMessage::ToggleFullscreen));
            }
            if args.presentation || cfg_presentation {
                startup_tasks =
                    startup_tasks.chain(iced::Task::done(AppMessage::TogglePresentationMode));
            }

            (state, startup_tasks)
        },
        App::update,
        App::view,
    )
    .title("Miro")
    .antialiasing(true)
    .theme(theme)
    .subscription(App::subscription)
    .window(settings())
    .font(include_bytes!("../assets/font/Geist-VariableFont_wght.ttf").as_slice())
    .default_font(Font::with_name("Geist"))
    .run()?)
}

pub fn theme(app: &App) -> Theme {
    use iced::theme::palette::{*};

    // TODO: Custom themes for UI, and maybe custom pdf background color (perhaps in json files?).
    let miro_light = Theme::custom_with_fn(
        "Miro Light".to_string(),
        iced::theme::Palette {
            background: Color::from_rgb8(240, 239, 238),
            text: Color::from_rgb8(30, 30, 30),
            primary: Color::from_rgb8(167, 143, 135),
            success: Color::from_rgb8(0, 255, 0),
            warning: Color::from_rgb8(255, 165, 0),
            danger: Color::from_rgb8(255, 0, 0),
        },
        |palette: Palette| {
            let bg = Background::new(palette.background, palette.text);
            let secondary = Secondary::generate(palette.background, palette.text);
            Extended {
                background: Background {
                    base: Pair {
                        color: Color::from_rgb8(240, 239, 238),
                        text: Color::from_rgb8(30, 30, 30),
                    },
                    weak: Pair {
                        color: Color::from_rgb8(255, 255, 255),
                        text: Color::from_rgb8(30, 30, 30),
                    },
                    strong: Pair {
                        color: Color::from_rgb8(187, 184, 187),
                        text: Color::from_rgb8(255, 255, 255),
                    },
                    weakest: bg.weakest,
                    weaker: bg.weaker,
                    neutral: bg.neutral,
                    stronger: bg.stronger,
                    strongest: bg.strongest,
                },
                primary: Primary {
                    base: Pair {
                        color: Color::from_rgb8(167, 143, 135),
                        text: Color::from_rgb8(255, 255, 255),
                    },
                    weak: Pair {
                        color: Color::from_rgb8(228, 226, 226),
                        text: Color::from_rgb8(255, 255, 255),
                    },
                    strong: Pair {
                        color: Color::from_rgb8(147, 123, 115),
                        text: Color::from_rgb8(255, 255, 255),
                    },
                },
                secondary: Secondary {
                    base: Pair {
                        color: Color::from_rgb8(217, 217, 217),
                        text: Color::from_rgb8(122, 122, 122),
                    },
                    weak: secondary.weak,
                    strong: secondary.strong,
                },
                success: Success::generate(palette.success, palette.background, palette.text),
                warning: Warning::generate(palette.warning, palette.background, palette.text),
                danger: Danger {
                    base: Pair {
                        color: Color::from_rgb8(167, 143, 135),
                        text: Color::from_rgb8(255, 255, 255),
                    },
                    weak: Pair {
                        color: Color::from_rgb8(228, 226, 226),
                        text: Color::from_rgb8(30, 30, 30),
                    },
                    strong: Pair {
                        color: Color::from_rgb8(147, 123, 115),
                        text: Color::from_rgb8(255, 255, 255),
                    },
                },
                is_dark: false,
            }
        },
    );
    let miro_dark = Theme::custom_with_fn(
        "Miro Dark".to_string(),
        iced::theme::Palette {
            background: Color::from_rgb8(26, 27, 38),
            text: Color::from_rgb8(154, 165, 206),
            primary: Color::from_rgb8(42, 195, 222),
            success: Color::from_rgb8(158, 206, 106),
            warning: Color::from_rgb8(255, 165, 0),
            danger: Color::from_rgb8(247, 118, 142),
        },
        |palette: Palette| {
            let bg = Background::new(palette.background, palette.text);
            Extended {
                background: Background {
                    base: Pair {
                        color: Color::from_rgb8(26, 27, 38),
                        text: Color::from_rgb8(154, 165, 206),
                    },
                    weak: Pair {
                        color: Color::from_rgb8(36, 40, 59),
                        text: Color::from_rgb8(154, 165, 206),
                    },
                    strong: Pair {
                        color: Color::from_rgb8(51, 56, 71),
                        text: Color::from_rgb8(154, 165, 206),
                    },
                    weakest: bg.weakest,
                    weaker: bg.weaker,
                    neutral: bg.neutral,
                    stronger: bg.stronger,
                    strongest: bg.strongest,
                },
                primary: Primary {
                    base: Pair {
                        color: Color::from_rgb8(42, 195, 222),
                        text: Color::from_rgb8(26, 27, 38),
                    },
                    weak: Pair {
                        color: Color::from_rgb8(73, 219, 240),
                        text: Color::from_rgb8(26, 27, 38),
                    },
                    strong: Pair {
                        color: Color::from_rgb8(21, 171, 204),
                        text: Color::from_rgb8(255, 255, 255),
                    },
                },
                secondary: Secondary {
                    base: Pair {
                        color: Color::from_rgb8(51, 56, 71),
                        text: Color::from_rgb8(154, 165, 206),
                    },
                    weak: Pair {
                        color: Color::from_rgb8(68, 75, 95),
                        text: Color::from_rgb8(154, 165, 206),
                    },
                    strong: Pair {
                        color: Color::from_rgb8(34, 39, 47),
                        text: Color::from_rgb8(154, 165, 206),
                    },
                },
                success: Success {
                    base: Pair {
                        color: Color::from_rgb8(158, 206, 106),
                        text: Color::from_rgb8(26, 27, 38),
                    },
                    weak: Pair {
                        color: Color::from_rgb8(180, 220, 140),
                        text: Color::from_rgb8(26, 27, 38),
                    },
                    strong: Pair {
                        color: Color::from_rgb8(136, 192, 72),
                        text: Color::from_rgb8(255, 255, 255),
                    },
                },
                warning: Warning::generate(palette.warning, palette.background, palette.text),
                danger: Danger {
                    base: Pair {
                        color: Color::from_rgb8(247, 118, 142),
                        text: Color::from_rgb8(26, 27, 38),
                    },
                    weak: Pair {
                        color: Color::from_rgb8(250, 150, 170),
                        text: Color::from_rgb8(26, 27, 38),
                    },
                    strong: Pair {
                        color: Color::from_rgb8(244, 86, 114),
                        text: Color::from_rgb8(255, 255, 255),
                    },
                },
                is_dark: true,
            }
        },
    );
    match app.dark_mode {
        true => miro_dark,
        false => miro_light,
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
