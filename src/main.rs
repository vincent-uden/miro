// Widgets to use from iced-aw
// - Menu (File -> Open/Save and so on)

use std::path::PathBuf;

use app::{App, AppMessage};
use clap::Parser;
use iced::Theme;

mod app;
mod custom_serde_functions;
mod pdf;

#[derive(Parser, Debug)]
#[command(version, name = "miro", about = "A pdf viewer")]
struct Args {
    #[arg(value_name = "PATH")]
    path: Option<PathBuf>,
}

fn main() -> iced::Result {
    let args = Args::parse();

    iced::application("App", App::update, App::view)
        .antialiasing(true)
        .theme(theme)
        .subscription(App::subscription)
        .run_with(|| {
            let mut state = App::new();
            match args.path {
                Some(p) => {
                    let _ = state.update(app::AppMessage::OpenFile(p));
                }
                None => {}
            }
            (state, iced::Task::none())
        })
}

pub fn theme(_: &App) -> Theme {
    Theme::Dark
}
// TODO: Why arent the two different subscriptions working nicely togather???
