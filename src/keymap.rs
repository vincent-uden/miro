use std::str::FromStr;

use keybinds::{Key, KeyInput, Keybind, Keybinds};
use logos::Logos;
use strum::EnumString;

use crate::{app::AppMessage, pdf::PdfMessage};

const MOVE_STEP: f32 = 40.0;

#[derive(Debug, EnumString, Clone, Copy, PartialEq, Eq)]
pub enum BindableMessage {
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
    NextPage,
    PreviousPage,
    ZoomHome,
    ZoomFit,
    ZoomIn,
    ZoomOut,
    NextTab,
    PreviousTab,
    ToggleDarkModePdf,
    DebugPrintImage,
}

impl From<BindableMessage> for AppMessage {
    fn from(val: BindableMessage) -> Self {
        match val {
            BindableMessage::MoveUp => AppMessage::PdfMessage(PdfMessage::MoveVertical(-MOVE_STEP)),
            BindableMessage::MoveDown => {
                AppMessage::PdfMessage(PdfMessage::MoveVertical(MOVE_STEP))
            }
            BindableMessage::MoveLeft => {
                AppMessage::PdfMessage(PdfMessage::MoveHorizontal(-MOVE_STEP))
            }
            BindableMessage::MoveRight => {
                AppMessage::PdfMessage(PdfMessage::MoveHorizontal(MOVE_STEP))
            }
            BindableMessage::NextPage => AppMessage::PdfMessage(PdfMessage::NextPage),
            BindableMessage::PreviousPage => AppMessage::PdfMessage(PdfMessage::PreviousPage),
            BindableMessage::ZoomHome => AppMessage::PdfMessage(PdfMessage::ZoomHome),
            BindableMessage::ZoomFit => AppMessage::PdfMessage(PdfMessage::ZoomFit),
            BindableMessage::ZoomIn => AppMessage::PdfMessage(PdfMessage::ZoomIn),
            BindableMessage::ZoomOut => AppMessage::PdfMessage(PdfMessage::ZoomOut),
            BindableMessage::NextTab => AppMessage::NextTab,
            BindableMessage::PreviousTab => AppMessage::PreviousTab,
            BindableMessage::ToggleDarkModePdf => AppMessage::ToggleDarkModePdf,
            BindableMessage::DebugPrintImage => AppMessage::DebugPrintImage,
        }
    }
}

#[derive(Debug)]
pub struct Config {
    pub keyboard: Keybinds<BindableMessage>,
}

impl Config {
    pub fn new() -> Self {
        Config {
            keyboard: Keybinds::new(vec![]),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            keyboard: Keybinds::new(vec![
                Keybind::new(KeyInput::from_str("j").unwrap(), BindableMessage::MoveDown),
                Keybind::new(KeyInput::from_str("k").unwrap(), BindableMessage::MoveUp),
                Keybind::new(KeyInput::from_str("h").unwrap(), BindableMessage::MoveLeft),
                Keybind::new(KeyInput::from_str("l").unwrap(), BindableMessage::MoveRight),
                Keybind::new(KeyInput::from_str("J").unwrap(), BindableMessage::NextPage),
                Keybind::new(
                    KeyInput::from_str("K").unwrap(),
                    BindableMessage::PreviousPage,
                ),
                Keybind::new(
                    KeyInput::from_str("H").unwrap(),
                    BindableMessage::PreviousTab,
                ),
                Keybind::new(KeyInput::from_str("L").unwrap(), BindableMessage::NextTab),
                Keybind::new(KeyInput::from_str("0").unwrap(), BindableMessage::ZoomHome),
                Keybind::new(KeyInput::from_str("_").unwrap(), BindableMessage::ZoomFit),
                Keybind::new(KeyInput::from_str("-").unwrap(), BindableMessage::ZoomOut),
                Keybind::new(KeyInput::from_str("Plus").unwrap(), BindableMessage::ZoomIn),
                Keybind::new(
                    KeyInput::from_str("Ctrl+r").unwrap(),
                    BindableMessage::ToggleDarkModePdf,
                ),
                Keybind::new(KeyInput::from(Key::F12), BindableMessage::DebugPrintImage),
            ]),
        }
    }
}

impl FromStr for Config {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let lexer = Token::lexer(s);

        let mut expecting_statement = true;

        let mut cmd_name = None;
        let mut args = vec![];

        let mut out = Config::new();

        // TODO: Count the line number to give error messages for each invalid line
        for token in lexer {
            match token {
                Ok(Token::String(s)) => {
                    if expecting_statement {
                        cmd_name = Some(s);
                    } else {
                        args.push(s);
                    }
                }
                Ok(Token::StatementDelim) => {
                    if let Some(Some(cmd)) = cmd_name.clone().map(|s| Command::from_str(&s).ok()) {
                        match cmd {
                            Command::Bind => {
                                assert!(args.len() == 2, "Bind requires two arguments");
                                out.keyboard
                                    .bind(&args[0], BindableMessage::from_str(&args[1]).unwrap())
                                    .unwrap();
                            }
                        }
                    } else {
                        todo!("Error handling for config parsing")
                    }
                    expecting_statement = true;
                    cmd_name = None;
                    args.clear();
                }
                Ok(Token::ArgDelim) => {
                    expecting_statement = false;
                }
                Err(e) => panic!("{:?}", e),
            }
        }
        Ok(out)
    }
}

/// Represents valid tokens in a configuration file.
#[derive(Debug, Logos)]
enum Token {
    #[regex(" +")]
    ArgDelim,

    #[token("\n")]
    StatementDelim,

    #[regex("[^ \n]+", |lex| lex.slice().to_owned())]
    String(String),
}

#[derive(Debug, EnumString)]
enum Command {
    Bind,
}

#[cfg(test)]
mod tests {
    use keybinds::{KeyInput, Keybind};

    use super::*;

    #[test]
    pub fn can_parse_vim_bindings() {
        let _config = Config {
            keyboard: Keybinds::new(vec![
                Keybind::new('K', BindableMessage::PreviousPage),
                Keybind::new('L', BindableMessage::NextTab),
                Keybind::new(
                    [
                        KeyInput::from_str("Ctrl+n").unwrap(),
                        KeyInput::from_str("Ctrl+w").unwrap(),
                        KeyInput::from_str("Ctrl+Plus").unwrap(),
                    ],
                    BindableMessage::NextTab,
                ),
            ]),
        };
    }

    #[test]
    pub fn can_parse_config_file() {
        let contents = include_str!("../assets/default.conf");
        let config = Config::from_str(contents).unwrap();
        let binds = config.keyboard.into_vec();
        let default_cfg = Config::default();
        let default_binds = default_cfg.keyboard.into_vec();
        assert_eq!(binds.len(), default_binds.len());
        for (b1, b2) in binds.iter().zip(default_binds) {
            assert_eq!(b1.seq, b2.seq);
            assert_eq!(b1.action, b2.action);
        }
    }
}
