use anyhow::{Result, anyhow};
use std::{fs, path::PathBuf, str::FromStr, fmt};

use colored::Colorize;
use keybinds::{KeyInput, KeySeq, Keybind, Keybinds};
use strum::EnumString;

use crate::{app::AppMessage, geometry::Vector, pdf::PdfMessage};

pub const MOVE_STEP: f32 = 40.0;

#[derive(Debug, Clone)]
pub struct ConfigError {
    pub line_number: usize,
    pub message: String,
}

impl ConfigError {
    pub fn new(line_number: usize, message: String) -> Self {
        Self {
            line_number,
            message,
        }
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {}: {}",
            "Line".bright_blue(),
            self.line_number.to_string().bright_yellow(),
            self.message.bright_red()
        )
    }
}

#[derive(Debug)]
pub struct ConfigParseResult {
    pub config: Config,
    pub errors: Vec<ConfigError>,
}

impl ConfigParseResult {
    pub fn new() -> Self {
        Self {
            config: Config::new(),
            errors: Vec::new(),
        }
    }

    pub fn add_error(&mut self, line_number: usize, message: String) {
        self.errors.push(ConfigError::new(line_number, message));
    }

    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    pub fn format_errors(&self) -> String {
        if self.errors.is_empty() {
            return String::new();
        }

        let mut output = format!("{}\n", "Configuration parsing errors:".bright_red().bold());
        for error in &self.errors {
            output.push_str(&format!("  {error}\n"));
        }
        output
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumString)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
    Back,
    Forward,
    ScrollUp,
    ScrollDown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct MouseModifiers {
    pub ctrl: bool,
    pub shift: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MouseInput {
    pub button: MouseButton,
    pub modifiers: MouseModifiers,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, EnumString, Default, serde::Serialize, serde::Deserialize,
)]
pub enum MouseAction {
    #[default]
    Panning,
    Selection,
    NextPage,
    PreviousPage,
    ZoomIn,
    ZoomOut,
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
}

pub type MouseBinding = (MouseInput, MouseAction);

impl FromStr for MouseInput {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split('+').collect();

        let mut modifiers = MouseModifiers::default();
        let mut button_str = s;

        // Parse modifiers
        if parts.len() > 1 {
            button_str = parts.last().unwrap();
            for part in &parts[..parts.len() - 1] {
                match *part {
                    "Ctrl" => modifiers.ctrl = true,
                    "Shift" => modifiers.shift = true,
                    _ => return Err(anyhow!("Unknown modifier: {}", part)),
                }
            }
        }

        // Strip "Mouse" prefix if present
        let button_name = if let Some(stripped) = button_str.strip_prefix("Mouse") {
            stripped
        } else {
            button_str
        };

        let button = MouseButton::from_str(button_name)?;

        Ok(MouseInput { button, modifiers })
    }
}

// Showing keybindings in menus
//
// There must be a link between each menu button and the corresponding, bound action
// That does inherently mean that each menu button needs to be able to be key-bound
// If that isn't desirable, each menu button could have an Option<BindableMessage> instead

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
    ToggleDarkModeUi,
    TogglePageBorders,
    ToggleSidebar,
    ToggleLinkHitboxes,
    OpenFileFinder,
    CloseTab,
    PrintPdf,
    Exit,
    JumpBack,
    JumpForward,
}

impl From<BindableMessage> for AppMessage {
    fn from(val: BindableMessage) -> Self {
        match val {
            BindableMessage::MoveUp => {
                AppMessage::PdfMessage(PdfMessage::Move(Vector::new(0.0, -MOVE_STEP)))
            }
            BindableMessage::MoveDown => {
                AppMessage::PdfMessage(PdfMessage::Move(Vector::new(0.0, MOVE_STEP)))
            }
            BindableMessage::MoveLeft => {
                AppMessage::PdfMessage(PdfMessage::Move(Vector::new(-MOVE_STEP, 0.0)))
            }
            BindableMessage::MoveRight => {
                AppMessage::PdfMessage(PdfMessage::Move(Vector::new(MOVE_STEP, 0.0)))
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
            BindableMessage::ToggleDarkModeUi => AppMessage::ToggleDarkModeUi,
            BindableMessage::TogglePageBorders => AppMessage::TogglePageBorders,
            BindableMessage::ToggleSidebar => AppMessage::ToggleSidebar,
            BindableMessage::ToggleLinkHitboxes => {
                AppMessage::PdfMessage(PdfMessage::ToggleLinkHitboxes)
            }
            BindableMessage::OpenFileFinder => AppMessage::OpenNewFileFinder,
            BindableMessage::CloseTab => AppMessage::CloseActiveTab,
            BindableMessage::PrintPdf => AppMessage::PdfMessage(PdfMessage::PrintPdf),
            BindableMessage::Exit => AppMessage::Exit,
            BindableMessage::JumpBack => AppMessage::JumpBack,
            BindableMessage::JumpForward => AppMessage::JumpForward,
        }
    }
}

#[derive(Debug)]
pub struct Config {
    pub keyboard: Keybinds<BindableMessage>,
    pub mouse: Vec<MouseBinding>,
    pub rpc_enabled: bool,
    pub rpc_port: u32,
    pub trackpad_sensitivity: f32,
    pub page_borders: bool,
    pub dark_mode: bool,
    pub invert_pdf: bool,
    pub open_sidebar: bool,
}

impl Config {
    pub fn new() -> Self {
        Config {
            keyboard: Keybinds::new(vec![]),
            mouse: Vec::new(),
            trackpad_sensitivity: 1.0,
            ..Default::default()
        }
    }

    pub fn get_binding_for_msg(&self, msg: BindableMessage) -> Option<Keybind<BindableMessage>> {
        let binds = self.keyboard.as_slice();
        binds.iter().find(|b| b.action == msg).cloned()
    }

    pub fn get_mouse_action(&self, input: MouseInput) -> Option<MouseAction> {
        self.mouse
            .iter()
            .find(|(mouse_input, _)| *mouse_input == input)
            .map(|(_, action)| *action)
    }

    pub fn system_config() -> Result<Self> {
        let config_path = Self::system_config_path()?;
        let content = fs::read_to_string(config_path)?;
        let parse_result = Self::parse_with_errors(&content);

        if parse_result.has_errors() {
            eprintln!("{}", parse_result.format_errors());
        }

        Ok(Self::merge_configs(Self::default(), &parse_result.config))
    }

    pub fn parse_with_errors(s: &str) -> ConfigParseResult {
        let mut result = ConfigParseResult::new();
        let lines: Vec<&str> = s.lines().collect();

        for (line_number, line) in lines.iter().enumerate() {
            let line_num = line_number + 1; // 1-based line numbers
            let trimmed = line.trim();

            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            if let Err(error) = Self::parse_line(trimmed, &mut result.config) {
                result.add_error(line_num, error);
            }
        }

        result
    }

    fn parse_line(line: &str, config: &mut Config) -> Result<(), String> {
        let parts = Self::parse_line_parts(line)?;
        if parts.is_empty() {
            return Ok(());
        }

        let command = match Command::from_str(&parts[0]) {
            Ok(cmd) => cmd,
            Err(_) => return Err(format!("Unknown command: {}", parts[0])),
        };

        match command {
            Command::Bind => {
                if parts.len() != 3 {
                    return Err(
                        "Bind command requires exactly 2 arguments: <key> <action>".to_string()
                    );
                }

                let key_str = &parts[1];
                let action_str = &parts[2];

                let action = BindableMessage::from_str(action_str)
                    .map_err(|_| format!("Unknown action: {action_str}"))?;

                config
                    .keyboard
                    .bind(key_str, action)
                    .map_err(|e| format!("Failed to bind key '{key_str}': {e}"))?;
            }
            Command::MouseBind => {
                if parts.len() != 3 {
                    return Err(
                        "MouseBind command requires exactly 2 arguments: <mouse_input> <action>"
                            .to_string(),
                    );
                }

                let mouse_input_str = &parts[1];
                let action_str = &parts[2];

                let mouse_input = MouseInput::from_str(mouse_input_str)
                    .map_err(|e| format!("Invalid mouse input '{mouse_input_str}': {e}"))?;

                let mouse_action = MouseAction::from_str(action_str)
                    .map_err(|_| format!("Unknown mouse action: {action_str}"))?;

                config.mouse.push((mouse_input, mouse_action));
            }
            Command::Set => {
                if parts.len() != 3 {
                    return Err(
                        "Set command requires exactly 2 arguments: <setting> <value>".to_string(),
                    );
                }

                let setting = &parts[1];
                let value = &parts[2];

                match setting.as_str() {
                    "DarkModePdf" => {
                        config.invert_pdf = Self::parse_boolean("DarkModePdf", value)?;
                    }
                    "DarkModeUi" => {
                        config.dark_mode = Self::parse_boolean("DarkModeUi", value)?;
                    }
                    "OpenSidebar" => {
                        config.open_sidebar = Self::parse_boolean("OpenSidebar", value)?;
                    }
                    "PageBorders" => {
                        config.page_borders = Self::parse_boolean("PageBorders", value)?;
                    }
                    "Rpc" => {
                        config.rpc_enabled = Self::parse_boolean("Rpc", value)?;
                    }
                    "RpcPort" => {
                        config.rpc_port = value.parse::<u32>().map_err(|_| {
                            format!("Invalid port number: '{value}'. Must be a valid integer")
                        })?;
                    }
                    "TrackpadSensitivity" => {
                        config.trackpad_sensitivity = value.parse::<f32>().map_err(|_| {
                            format!("Invalid float value for TrackpadSensitivity: '{value}'. Must be a valid number")
                        })?;
                    }
                    _ => return Err(format!("Unknown setting: {setting}")),
                }
            }
        }

        Ok(())
    }

    fn parse_boolean(value_name: &'static str, value: &str) -> Result<bool, String> {
        match value {
            "True" | "true" | "1" => Ok(true),
            "False" | "false" | "0" => Ok(false),
            _ => Err(format!(
                "Invalid boolean value for {value_name}: '{value}'. Use True/False"
            )),
        }
    }

    fn parse_line_parts(line: &str) -> Result<Vec<String>, String> {
        let mut parts = Vec::new();
        let mut current_part = String::new();
        let mut in_quotes = false;
        let chars = line.chars();

        for ch in chars {
            match ch {
                '"' => {
                    in_quotes = !in_quotes;
                }
                ' ' | '\t' if !in_quotes => {
                    if !current_part.is_empty() {
                        parts.push(current_part.clone());
                        current_part.clear();
                    }
                }
                _ => {
                    current_part.push(ch);
                }
            }
        }

        if in_quotes {
            return Err("Unterminated quoted string".to_string());
        }

        if !current_part.is_empty() {
            parts.push(current_part);
        }

        Ok(parts)
    }

    pub fn system_config_path() -> Result<PathBuf> {
        Ok(home::home_dir()
            .ok_or(anyhow!("No home directory could be determined"))?
            .join("./.config/miro-pdf/miro.conf"))
    }

    fn merge_configs(mut base: Config, overrider: &Config) -> Config {
        for binding in overrider.keyboard.as_slice() {
            base.keyboard.push(binding.clone());
        }
        for binding in &overrider.mouse {
            println!("{binding:?}");
            base.mouse.push(*binding);
        }
        base.rpc_enabled = overrider.rpc_enabled;
        base.rpc_port = overrider.rpc_port;
        base.trackpad_sensitivity = overrider.trackpad_sensitivity;
        base.page_borders = overrider.page_borders;
        base.dark_mode = overrider.dark_mode;
        base.invert_pdf = overrider.invert_pdf;
        base.open_sidebar = overrider.open_sidebar;
        base
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
                Keybind::new(
                    KeyInput::from_str("Alt+Left").unwrap(),
                    BindableMessage::JumpBack,
                ),
                Keybind::new(
                    KeyInput::from_str("Alt+Right").unwrap(),
                    BindableMessage::JumpForward,
                ),
                Keybind::new(KeyInput::from_str("0").unwrap(), BindableMessage::ZoomHome),
                Keybind::new(KeyInput::from_str("_").unwrap(), BindableMessage::ZoomFit),
                Keybind::new(KeyInput::from_str("-").unwrap(), BindableMessage::ZoomOut),
                Keybind::new(KeyInput::from_str("Plus").unwrap(), BindableMessage::ZoomIn),
                Keybind::new(
                    KeyInput::from_str("Ctrl+r").unwrap(),
                    BindableMessage::ToggleDarkModePdf,
                ),
                Keybind::new(
                    KeyInput::from_str("Ctrl+i").unwrap(),
                    BindableMessage::ToggleDarkModeUi,
                ),
                Keybind::new(
                    KeyInput::from_str("Ctrl+b").unwrap(),
                    BindableMessage::ToggleSidebar,
                ),
                Keybind::new(
                    KeyInput::from_str("Ctrl+l").unwrap(),
                    BindableMessage::ToggleLinkHitboxes,
                ),
                Keybind::new(
                    KeyInput::from_str("Ctrl+k").unwrap(),
                    BindableMessage::TogglePageBorders,
                ),
                Keybind::new(
                    KeyInput::from_str("Ctrl+o").unwrap(),
                    BindableMessage::OpenFileFinder,
                ),
                Keybind::new(
                    KeyInput::from_str("Ctrl+p").unwrap(),
                    BindableMessage::PrintPdf,
                ),
                Keybind::new(KeySeq::from_str("Z Z").unwrap(), BindableMessage::CloseTab),
                Keybind::new(KeySeq::from_str("q").unwrap(), BindableMessage::Exit),
            ]),
            mouse: vec![
                (
                    MouseInput {
                        button: MouseButton::Left,
                        modifiers: MouseModifiers {
                            ctrl: false,
                            shift: false,
                        },
                    },
                    MouseAction::Panning,
                ),
                (
                    MouseInput {
                        button: MouseButton::Left,
                        modifiers: MouseModifiers {
                            ctrl: false,
                            shift: true,
                        },
                    },
                    MouseAction::Selection,
                ),
                (
                    MouseInput {
                        button: MouseButton::Middle,
                        modifiers: MouseModifiers {
                            ctrl: false,
                            shift: false,
                        },
                    },
                    MouseAction::Panning,
                ),
                (
                    MouseInput {
                        button: MouseButton::Right,
                        modifiers: MouseModifiers {
                            ctrl: false,
                            shift: false,
                        },
                    },
                    MouseAction::Selection,
                ),
                (
                    MouseInput {
                        button: MouseButton::Forward,
                        modifiers: MouseModifiers {
                            ctrl: false,
                            shift: false,
                        },
                    },
                    MouseAction::NextPage,
                ),
                (
                    MouseInput {
                        button: MouseButton::Back,
                        modifiers: MouseModifiers {
                            ctrl: false,
                            shift: false,
                        },
                    },
                    MouseAction::PreviousPage,
                ),
                (
                    MouseInput {
                        button: MouseButton::ScrollUp,
                        modifiers: MouseModifiers::default(),
                    },
                    MouseAction::MoveUp,
                ),
                (
                    MouseInput {
                        button: MouseButton::ScrollDown,
                        modifiers: MouseModifiers::default(),
                    },
                    MouseAction::MoveDown,
                ),
                (
                    MouseInput {
                        button: MouseButton::ScrollUp,
                        modifiers: MouseModifiers {
                            ctrl: true,
                            shift: false,
                        },
                    },
                    MouseAction::ZoomIn,
                ),
                (
                    MouseInput {
                        button: MouseButton::ScrollDown,
                        modifiers: MouseModifiers {
                            ctrl: true,
                            shift: false,
                        },
                    },
                    MouseAction::ZoomOut,
                ),
                (
                    MouseInput {
                        button: MouseButton::ScrollUp,
                        modifiers: MouseModifiers {
                            ctrl: false,
                            shift: true,
                        },
                    },
                    MouseAction::MoveLeft,
                ),
                (
                    MouseInput {
                        button: MouseButton::ScrollDown,
                        modifiers: MouseModifiers {
                            ctrl: false,
                            shift: true,
                        },
                    },
                    MouseAction::MoveRight,
                ),
            ],
            rpc_enabled: false,
            rpc_port: 7890,
            trackpad_sensitivity: 1.0,
            page_borders: true,
            dark_mode: true,
            invert_pdf: false,
            open_sidebar: false,
        }
    }
}

impl FromStr for Config {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parse_result = Self::parse_with_errors(s);

        if parse_result.has_errors() {
            return Err(anyhow!("{}", parse_result.format_errors()));
        }

        Ok(parse_result.config)
    }
}

#[derive(Debug, EnumString)]
enum Command {
    Bind,
    MouseBind,
    Set,
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
            mouse: Vec::new(),
            rpc_enabled: false,
            rpc_port: 7890,
            trackpad_sensitivity: 1.0,
            page_borders: true,
            dark_mode: true,
            invert_pdf: false,
            open_sidebar: false,
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

        // Check mouse bindings
        assert_eq!(config.mouse.len(), default_cfg.mouse.len());
        for (b1, b2) in config.mouse.iter().zip(default_cfg.mouse.iter()) {
            assert_eq!(b1.0, b2.0); // MouseInput
            assert_eq!(b1.1, b2.1); // MouseAction
        }

        // Check other settings
        assert_eq!(config.rpc_enabled, default_cfg.rpc_enabled);
        assert_eq!(config.rpc_port, default_cfg.rpc_port);
        assert_eq!(
            config.trackpad_sensitivity,
            default_cfg.trackpad_sensitivity
        );
    }

    #[test]
    pub fn can_parse_mouse_input() {
        // Test basic mouse buttons
        let input = MouseInput::from_str("Left").unwrap();
        assert_eq!(input.button, MouseButton::Left);
        assert_eq!(input.modifiers, MouseModifiers::default());

        let input = MouseInput::from_str("Middle").unwrap();
        assert_eq!(input.button, MouseButton::Middle);

        let input = MouseInput::from_str("Right").unwrap();
        assert_eq!(input.button, MouseButton::Right);

        // Test with modifiers
        let input = MouseInput::from_str("Ctrl+Left").unwrap();
        assert_eq!(input.button, MouseButton::Left);
        assert_eq!(input.modifiers.ctrl, true);
        assert_eq!(input.modifiers.shift, false);

        let input = MouseInput::from_str("Shift+Right").unwrap();
        assert_eq!(input.button, MouseButton::Right);
        assert_eq!(input.modifiers.ctrl, false);
        assert_eq!(input.modifiers.shift, true);

        let input = MouseInput::from_str("Ctrl+Shift+Middle").unwrap();
        assert_eq!(input.button, MouseButton::Middle);
        assert_eq!(input.modifiers.ctrl, true);
        assert_eq!(input.modifiers.shift, true);
    }

    #[test]
    pub fn can_get_mouse_action() {
        let config = Config::default();

        let input = MouseInput {
            button: MouseButton::Left,
            modifiers: MouseModifiers::default(),
        };
        assert_eq!(config.get_mouse_action(input), Some(MouseAction::Panning));

        let input = MouseInput {
            button: MouseButton::Left,
            modifiers: MouseModifiers {
                ctrl: false,
                shift: true,
            },
        };
        assert_eq!(config.get_mouse_action(input), Some(MouseAction::Selection));

        let input = MouseInput {
            button: MouseButton::Middle,
            modifiers: MouseModifiers::default(),
        };
        assert_eq!(config.get_mouse_action(input), Some(MouseAction::Panning));

        let input = MouseInput {
            button: MouseButton::Right,
            modifiers: MouseModifiers {
                ctrl: true,
                shift: false,
            },
        };
        assert_eq!(config.get_mouse_action(input), None);
    }

    #[test]
    pub fn error_handling_unknown_command() {
        let config_str = "UnknownCommand arg1 arg2";
        let result = Config::parse_with_errors(config_str);

        assert!(result.has_errors());
        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.errors[0].line_number, 1);
        assert!(
            result.errors[0]
                .message
                .contains("Unknown command: UnknownCommand")
        );
    }

    #[test]
    pub fn error_handling_invalid_bind_args() {
        let config_str = "Bind j";
        let result = Config::parse_with_errors(config_str);

        assert!(result.has_errors());
        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.errors[0].line_number, 1);
        assert!(
            result.errors[0]
                .message
                .contains("Bind command requires exactly 2 arguments")
        );
    }

    #[test]
    pub fn error_handling_invalid_action() {
        let config_str = "Bind j InvalidAction";
        let result = Config::parse_with_errors(config_str);

        assert!(result.has_errors());
        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.errors[0].line_number, 1);
        assert!(
            result.errors[0]
                .message
                .contains("Unknown action: InvalidAction")
        );
    }

    #[test]
    pub fn error_handling_invalid_mouse_input() {
        let config_str = "MouseBind InvalidMouse Panning";
        let result = Config::parse_with_errors(config_str);

        assert!(result.has_errors());
        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.errors[0].line_number, 1);
        assert!(
            result.errors[0]
                .message
                .contains("Invalid mouse input 'InvalidMouse'")
        );
    }

    #[test]
    pub fn error_handling_invalid_set_value() {
        let config_str = "Set RpcPort invalid_port";
        let result = Config::parse_with_errors(config_str);

        assert!(result.has_errors());
        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.errors[0].line_number, 1);
        assert!(
            result.errors[0]
                .message
                .contains("Invalid port number: 'invalid_port'")
        );
    }

    #[test]
    pub fn error_handling_multiple_errors() {
        let config_str = r#"
Bind j InvalidAction
UnknownCommand arg1
Set RpcPort invalid_port
Bind k MoveUp
MouseBind InvalidMouse Panning
"#;
        let result = Config::parse_with_errors(config_str);

        assert!(result.has_errors());
        assert_eq!(result.errors.len(), 4);

        // Check that valid lines are still processed
        assert!(!result.config.keyboard.as_slice().is_empty());
    }

    #[test]
    pub fn error_handling_unterminated_quotes() {
        let config_str = r#"Bind "unterminated quote MoveUp"#;
        let result = Config::parse_with_errors(config_str);

        assert!(result.has_errors());
        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.errors[0].line_number, 1);
        assert!(
            result.errors[0]
                .message
                .contains("Unterminated quoted string")
        );
    }

    #[test]
    pub fn error_handling_skips_comments_and_empty_lines() {
        let config_str = r#"
# This is a comment
Bind j MoveDown

# Another comment
Bind k MoveUp
"#;
        let result = Config::parse_with_errors(config_str);

        assert!(!result.has_errors());
        assert_eq!(result.config.keyboard.as_slice().len(), 2);
    }

    #[test]
    pub fn demonstrate_colored_error_output() {
        use colored::control;

        // Disable colors for consistent testing
        control::set_override(false);

        let config_str = r#"
Bind j InvalidAction
UnknownCommand arg1
Set RpcPort invalid_port
"#;
        let result = Config::parse_with_errors(config_str);

        assert!(result.has_errors());
        assert_eq!(result.errors.len(), 3);

        // Print the colored output for manual verification
        // This won't show colors in test output, but demonstrates the functionality
        let formatted = result.format_errors();
        println!("\n{}", formatted);

        // Verify the content is correct
        assert!(formatted.contains("Configuration parsing errors:"));
        assert!(formatted.contains("Line 2:"));
        assert!(formatted.contains("Line 3:"));
        assert!(formatted.contains("Line 4:"));

        // Re-enable colors
        control::unset_override();
    }

    #[test]
    pub fn can_parse_trackpad_sensitivity() {
        let config_str = "Set TrackpadSensitivity 0.5";
        let result = Config::parse_with_errors(config_str);

        assert!(!result.has_errors());
        assert_eq!(result.config.trackpad_sensitivity, 0.5);
    }

    #[test]
    pub fn error_handling_invalid_trackpad_sensitivity() {
        let config_str = "Set TrackpadSensitivity invalid";
        let result = Config::parse_with_errors(config_str);

        assert!(result.has_errors());
        assert_eq!(result.errors.len(), 1);
        assert!(
            result.errors[0]
                .message
                .contains("Invalid float value for TrackpadSensitivity")
        );
    }

    #[test]
    pub fn test_config_file_with_errors() {
        use std::fs;

        let config_content = fs::read_to_string("test_config_with_errors.conf");
        if let Ok(content) = config_content {
            let result = Config::parse_with_errors(&content);

            if result.has_errors() {
                // This will show colored output when run with --nocapture
                eprintln!("{}", result.format_errors());
            }

            // Should still parse valid lines
            assert!(!result.config.keyboard.as_slice().is_empty());
        }
    }
}
