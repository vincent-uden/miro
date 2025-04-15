use std::str::FromStr;

use keybinds::{KeyInput, Keybind, Keybinds};

use crate::{app::AppMessage, pdf::PdfMessage};

const MOVE_STEP: f32 = 40.0;

#[derive(Debug)]
pub struct Config {
    pub keyboard: Keybinds<AppMessage>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            keyboard: Keybinds::new(vec![
                Keybind::new(
                    KeyInput::from_str("j").unwrap(),
                    AppMessage::PdfMessage(PdfMessage::MoveVertical(MOVE_STEP)),
                ),
                Keybind::new(
                    KeyInput::from_str("k").unwrap(),
                    AppMessage::PdfMessage(PdfMessage::MoveVertical(-MOVE_STEP)),
                ),
                Keybind::new(
                    KeyInput::from_str("h").unwrap(),
                    AppMessage::PdfMessage(PdfMessage::MoveHorizontal(-MOVE_STEP)),
                ),
                Keybind::new(
                    KeyInput::from_str("l").unwrap(),
                    AppMessage::PdfMessage(PdfMessage::MoveHorizontal(MOVE_STEP)),
                ),
                Keybind::new(
                    KeyInput::from_str("J").unwrap(),
                    AppMessage::PdfMessage(PdfMessage::NextPage),
                ),
                Keybind::new(
                    KeyInput::from_str("K").unwrap(),
                    AppMessage::PdfMessage(PdfMessage::PreviousPage),
                ),
                Keybind::new(KeyInput::from_str("H").unwrap(), AppMessage::PreviousTab),
                Keybind::new(KeyInput::from_str("L").unwrap(), AppMessage::NextTab),
                Keybind::new(
                    KeyInput::from_str("0").unwrap(),
                    AppMessage::PdfMessage(PdfMessage::ZoomHome),
                ),
                Keybind::new(
                    KeyInput::from_str("_").unwrap(),
                    AppMessage::PdfMessage(PdfMessage::ZoomFit),
                ),
                Keybind::new(
                    KeyInput::from_str("-").unwrap(),
                    AppMessage::PdfMessage(PdfMessage::ZoomOut),
                ),
                Keybind::new(
                    KeyInput::from_str("Plus").unwrap(),
                    AppMessage::PdfMessage(PdfMessage::ZoomIn),
                ),
            ]),
        }
    }
}

#[cfg(test)]
mod tests {
    use keybinds::{KeyInput, KeySeq, Keybind, serde};

    use super::*;

    #[test]
    pub fn can_parse_vim_bindings() {
        let _config = Config {
            keyboard: Keybinds::new(vec![
                Keybind::new('K', AppMessage::PdfMessage(PdfMessage::PreviousPage)),
                Keybind::new('L', AppMessage::NextTab),
                Keybind::new(
                    [
                        KeyInput::from_str("Ctrl+n").unwrap(),
                        KeyInput::from_str("Ctrl+w").unwrap(),
                        KeyInput::from_str("Ctrl+Plus").unwrap(),
                    ],
                    AppMessage::NextTab,
                ),
            ]),
        };
    }
}
