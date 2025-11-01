#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    fs, io,
    path::PathBuf,
    sync::{LazyLock, RwLock},
};

use app::App;
use bookmarks::BookmarkStore;
use clap::Parser;
use config::Config;
use iced::{
    window::{get_latest, icon::from_file_data},
    Color, Font, Theme,
};
use tracing::info;
use tracing_subscriber::EnvFilter;

mod app;
mod bookmarks;
mod config;
mod geometry;
mod icons;
mod jumplist;
mod pdf;
mod rpc;
mod watch;

const DARK_THEME: Theme = Theme::TokyoNight;

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

    iced::application("Miro", App::update, App::view)
        .antialiasing(true)
        .theme(theme)
        .font(iced_fonts::REQUIRED_FONT_BYTES)
        .subscription(App::subscription)
        .window(settings())
        .font(include_bytes!("../assets/font/Geist-VariableFont_wght.ttf").as_slice())
        .default_font(Font::with_name("Geist"))
        .run_with(move || {
            let state = App::new(BookmarkStore::system_store().unwrap_or_default());
            let file_task = match args.path {
                Some(p) => iced::Task::done(app::AppMessage::OpenFile(p)),
                None => iced::Task::none(),
            };
            let file_task = file_task.chain(get_latest().map(app::AppMessage::FoundWindowId));

            (state, file_task)
        })
}

pub fn theme(app: &App) -> Theme {
    use iced::theme::palette::{*};

    let not_defined = Pair {
        color: Color::from_rgb8(255, 0, 23),
        text: Color::from_rgb8(13, 255, 0),
    };

    let miro_light = Theme::custom_with_fn(
        "Miro Light".to_string(),
        iced::theme::Palette {
            background: Color::from_rgb8(240, 239, 238),
            text: Color::from_rgb8(30, 30, 30),
            primary: Color::from_rgb8(167, 143, 135),
            success: Color::from_rgb8(0, 255, 0),
            danger: Color::from_rgb8(255, 0, 0),
        },
        |_: Palette| Extended {
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
                base: not_defined,
                weak: not_defined,
                strong: not_defined,
            },
            success: Success {
                base: not_defined,
                weak: not_defined,
                strong: not_defined,
            },
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
        },
    );
    let miro_dark = Theme::custom_with_fn(
        "Miro Dark".to_string(),
        iced::theme::Palette {
            background: Color::from_rgb8(26, 27, 38),
            text: Color::from_rgb8(154, 165, 206),
            primary: Color::from_rgb8(42, 195, 222),
            success: Color::from_rgb8(158, 206, 106),
            danger: Color::from_rgb8(247, 118, 142),
        },
        |_: Palette| Extended {
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
