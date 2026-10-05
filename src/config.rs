use anyhow::{Result, anyhow};
use std::{collections::HashMap, fmt, fs, path::PathBuf, str::FromStr};

use colored::Colorize;
use keybinds2::{KeyInput, KeySeq, Keybind, Keybinds};
use strum::{Display, EnumString};

use crate::{
    app::AppMessage,
    geometry::Vector,
    pdf::{PdfMessage, SearchMethod, page_layout::PageLayoutKind},
};

pub const MOVE_STEP: f32 = 40.0;

#[derive(Debug, Clone)]
pub struct ConfigError {
    pub line_number: usize,
    pub message: String,
    pub is_warning: bool,
}

impl ConfigError {
    pub fn new(line_number: usize, message: String) -> Self {
        Self {
            line_number,
            message,
            is_warning: false,
        }
    }

    pub fn warning(line_number: usize, message: String) -> Self {
        Self {
            line_number,
            message,
            is_warning: true,
        }
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = if self.is_warning {
            self.message.bright_yellow()
        } else {
            self.message.bright_red()
        };
        write!(
            f,
            "{} {}: {}",
            "Line".bright_blue(),
            self.line_number.to_string().bright_yellow(),
            message
        )
    }
}

#[derive(Debug)]
pub struct ConfigParseResult {
    pub config: Config,
    /// File the config was loaded from, if known. Used to give the error
    /// and warning output a provenance header.
    pub source: Option<PathBuf>,
    pub errors: Vec<ConfigError>,
    pub warnings: Vec<ConfigError>,
}

impl ConfigParseResult {
    pub fn new() -> Self {
        Self {
            config: Config::new(),
            source: None,
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub fn set_source(&mut self, source: PathBuf) {
        self.source = Some(source);
    }

    fn source_suffix(&self) -> String {
        match &self.source {
            Some(path) => format!(" in {}", path.display()),
            None => String::new(),
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

        let mut output = format!(
            "{}\n",
            format!("Configuration parsing errors{}:", self.source_suffix())
                .bright_red()
                .bold()
        );
        for error in &self.errors {
            output.push_str(&format!("  {error}\n"));
        }
        output
    }

    pub fn add_warning(&mut self, line_number: usize, message: String) {
        self.warnings
            .push(ConfigError::warning(line_number, message));
    }

    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }

    pub fn format_warnings(&self) -> String {
        if self.warnings.is_empty() {
            return String::new();
        }

        let mut output = format!(
            "{}\n",
            format!("Configuration parsing warnings{}:", self.source_suffix())
                .bright_yellow()
                .bold()
        );
        for warning in &self.warnings {
            output.push_str(&format!("  {warning}\n"));
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

#[derive(Debug, EnumString, Display, Clone, Copy, PartialEq, Eq)]
pub enum BindableMessage {
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
    NextPage,
    PreviousPage,
    PageUp,
    PageDown,
    HalfPageUp,
    HalfPageDown,
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
    #[strum(serialize = "CloseActiveTab", serialize = "CloseTab")]
    CloseTab,
    PrintPdf,
    Exit,
    JumpBack,
    JumpForward,
    ToggleFullscreen,
    TogglePresentationMode,
    OpenSearch,
    CloseSearch,
    ToggleSearchMethod,
    NextSearchResult,
    PreviousSearchResult,
    SinglePageLayout,
    DoublePageLayout,
    DoublePageTitlePageLayout,
    PresentationLayout,
    RotatePageClockwise,
    RotatePageCounterClockwise,
    RotateAllPagesClockwise,
    RotateAllPagesCounterClockwise,
}

impl BindableMessage {
    pub fn default_menu_label(&self) -> Option<&'static str> {
        match self {
            BindableMessage::OpenFileFinder => Some("Open File"),
            BindableMessage::PrintPdf => Some("Print"),
            BindableMessage::CloseTab => Some("Close"),
            BindableMessage::ToggleDarkModeUi => Some("Toggle Interface Dark Mode"),
            BindableMessage::ToggleDarkModePdf => Some("Toggle PDF Dark Mode"),
            BindableMessage::TogglePageBorders => Some("Toggle Page Borders"),
            BindableMessage::ZoomIn => Some("Zoom In"),
            BindableMessage::ZoomOut => Some("Zoom Out"),
            BindableMessage::ZoomHome => Some("Zoom 100%"),
            BindableMessage::ZoomFit => Some("Fit To Screen"),
            BindableMessage::ToggleSidebar => Some("Toggle Sidebar"),
            BindableMessage::TogglePresentationMode => Some("Presentation Mode"),
            BindableMessage::ToggleFullscreen => Some("Toggle Fullscreen"),
            BindableMessage::SinglePageLayout => Some("Single Page"),
            BindableMessage::DoublePageLayout => Some("Double Page"),
            BindableMessage::DoublePageTitlePageLayout => Some("Double Page w/ Title"),
            BindableMessage::PresentationLayout => Some("Presentation"),
            BindableMessage::RotatePageClockwise => Some("Rotate Page Clockwise"),
            BindableMessage::RotatePageCounterClockwise => Some("Rotate Page Counterclockwise"),
            BindableMessage::RotateAllPagesClockwise => Some("Rotate All Pages Clockwise"),
            BindableMessage::RotateAllPagesCounterClockwise => {
                Some("Rotate All Pages Counterclockwise")
            }
            _ => None,
        }
    }
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
            BindableMessage::ToggleFullscreen => AppMessage::ToggleFullscreen,
            BindableMessage::TogglePresentationMode => AppMessage::TogglePresentationMode,
            BindableMessage::OpenSearch => AppMessage::OpenSearch,
            BindableMessage::CloseSearch => AppMessage::CloseSearch,
            BindableMessage::ToggleSearchMethod => AppMessage::ToggleSearchMethod,
            BindableMessage::NextSearchResult => {
                AppMessage::PdfMessage(PdfMessage::NextSearchResult)
            }
            BindableMessage::PreviousSearchResult => {
                AppMessage::PdfMessage(PdfMessage::PreviousSearchResult)
            }
            BindableMessage::SinglePageLayout => {
                AppMessage::PdfMessage(PdfMessage::SetLayout(PageLayoutKind::SinglePage))
            }
            BindableMessage::DoublePageLayout => {
                AppMessage::PdfMessage(PdfMessage::SetLayout(PageLayoutKind::DoublePage))
            }
            BindableMessage::DoublePageTitlePageLayout => {
                AppMessage::PdfMessage(PdfMessage::SetLayout(PageLayoutKind::DoublePageTitlePage))
            }
            BindableMessage::PresentationLayout => {
                AppMessage::PdfMessage(PdfMessage::SetLayout(PageLayoutKind::Presentation))
            }
            BindableMessage::RotatePageClockwise => {
                AppMessage::PdfMessage(PdfMessage::RotatePageClockwise)
            }
            BindableMessage::RotatePageCounterClockwise => {
                AppMessage::PdfMessage(PdfMessage::RotatePageCounterClockwise)
            }
            BindableMessage::RotateAllPagesClockwise => {
                AppMessage::PdfMessage(PdfMessage::RotateAllPagesClockwise)
            }
            BindableMessage::RotateAllPagesCounterClockwise => {
                AppMessage::PdfMessage(PdfMessage::RotateAllPagesCounterClockwise)
            }
            BindableMessage::PageUp => AppMessage::PdfMessage(PdfMessage::PageUp),
            BindableMessage::PageDown => AppMessage::PdfMessage(PdfMessage::PageDown),
            BindableMessage::HalfPageUp => AppMessage::PdfMessage(PdfMessage::HalfPageUp),
            BindableMessage::HalfPageDown => AppMessage::PdfMessage(PdfMessage::HalfPageDown),
        }
    }
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, EnumString, Display,
)]
pub enum BindingMode {
    #[default]
    Normal,
    Presentation,
}

#[derive(Debug)]
pub struct Config {
    pub binding_mode: BindingMode,
    pub keyboard: HashMap<BindingMode, Keybinds<BindableMessage>>,
    pub mouse: HashMap<BindingMode, Vec<MouseBinding>>,
    pub rpc_enabled: bool,
    pub rpc_allow_lan: bool,
    pub rpc_port: u32,
    pub trackpad_sensitivity: f32,
    pub page_borders: bool,
    pub dark_mode: bool,
    pub invert_pdf: bool,
    pub open_sidebar: bool,
    pub default_search_method: SearchMethod,
    pub open_fullscreen_default: bool,
    pub open_presentation_default: bool,
}

impl Config {
    pub fn new() -> Self {
        let mut keyboard = HashMap::new();
        keyboard.insert(BindingMode::Normal, Keybinds::new(vec![]));
        keyboard.insert(BindingMode::Presentation, Keybinds::new(vec![]));

        let mut mouse = HashMap::new();
        mouse.insert(BindingMode::Normal, vec![]);
        mouse.insert(BindingMode::Presentation, vec![]);
        Config {
            keyboard,
            mouse,
            trackpad_sensitivity: 1.0,
            ..Default::default()
        }
    }
    pub fn get_binding_for_msg(&self, msg: BindableMessage) -> Option<Keybind<BindableMessage>> {
        let binds = self.keyboard[&self.binding_mode].as_slice();
        binds.iter().find(|b| b.action == msg).cloned()
    }

    pub fn get_mouse_action(&self, input: MouseInput) -> Option<MouseAction> {
        self.mouse[&self.binding_mode]
            .iter()
            .find(|(mouse_input, _)| *mouse_input == input)
            .map(|(_, action)| *action)
    }

    pub fn system_config() -> Result<Self> {
        let config_path = Self::system_config_path()?;
        let content = fs::read_to_string(&config_path)?;
        let config_path = canonize_path(config_path);
        let mut parse_result = Self::parse_with_errors(&content);
        parse_result.set_source(config_path);

        if parse_result.has_errors() {
            eprintln!("{}", parse_result.format_errors());
        }

        if parse_result.has_warnings() {
            eprintln!("{}", parse_result.format_warnings());
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

            if let Err(error) = Self::parse_line(trimmed, line_num, &mut result) {
                result.add_error(line_num, error);
            }
        }

        result
    }

    fn parse_line(
        line: &str,
        line_number: usize,
        result: &mut ConfigParseResult,
    ) -> Result<(), String> {
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
                if parts.len() != 3 && parts.len() != 4 {
                    return Err(
                        "Bind command requires 3 arguments: <key> <mode> <action>".to_string()
                    );
                }

                let (mode, action_str) = if parts.len() == 4 {
                    (
                        BindingMode::from_str(&parts[2])
                            .map_err(|_| format!("Unknown mode: {}", parts[2]))?,
                        &parts[3],
                    )
                } else {
                    result.add_warning(
                        line_number,
                        "Deprecated: Bind command without a mode defaults to Normal. \
                         Add the mode explicitly: Bind <key> Normal <action>"
                            .to_string(),
                    );
                    (BindingMode::Normal, &parts[2])
                };

                let key_str = &parts[1];

                let action = BindableMessage::from_str(action_str)
                    .map_err(|_| format!("Unknown action: {action_str}"))?;

                result
                    .config
                    .keyboard
                    .entry(mode)
                    .or_default()
                    .bind(key_str, action)
                    .map_err(|e| format!("Failed to bind key '{key_str}': {e}"))?;
            }
            Command::MouseBind => {
                if parts.len() != 3 && parts.len() != 4 {
                    return Err(
                        "MouseBind command requires 3 arguments: <mouse_input> <mode> <action>"
                            .to_string(),
                    );
                }

                let (mode, action_str) = if parts.len() == 4 {
                    (
                        BindingMode::from_str(&parts[2])
                            .map_err(|_| format!("Unknown mode: {}", parts[2]))?,
                        &parts[3],
                    )
                } else {
                    result.add_warning(
                        line_number,
                        "Deprecated: MouseBind command without a mode defaults to Normal. \
                         Add the mode explicitly: MouseBind <mouse_input> Normal <action>"
                            .to_string(),
                    );
                    (BindingMode::Normal, &parts[2])
                };

                let mouse_input_str = &parts[1];

                let mouse_input = MouseInput::from_str(mouse_input_str)
                    .map_err(|e| format!("Invalid mouse input '{mouse_input_str}': {e}"))?;

                let mouse_action = MouseAction::from_str(action_str)
                    .map_err(|_| format!("Unknown mouse action: {action_str}"))?;

                result
                    .config
                    .mouse
                    .entry(mode)
                    .or_default()
                    .push((mouse_input, mouse_action));
            }
            Command::Set => {
                let config = &mut result.config;
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
                    "RpcAllowLan" => {
                        config.rpc_allow_lan = Self::parse_boolean("RpcAllowLan", value)?;
                    }
                    "TrackpadSensitivity" => {
                        config.trackpad_sensitivity = value.parse::<f32>().map_err(|_| {
                            format!("Invalid float value for TrackpadSensitivity: '{value}'. Must be a valid number")
                        })?;
                    }
                    "DefaultSearchMethod" => {
                        config.default_search_method =
                            SearchMethod::from_str(value).map_err(|_| {
                                format!("Unknown search method: '{value}'. Use PlainText or Regex")
                            })?;
                    }
                    "OpenFullscreen" => {
                        config.open_fullscreen_default =
                            Self::parse_boolean("OpenFullscreen", value)?;
                    }
                    "OpenPresentation" => {
                        config.open_presentation_default =
                            Self::parse_boolean("OpenPresentation", value)?;
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
        for (mode, binds) in &overrider.keyboard {
            let base_binds = base.keyboard.entry(*mode).or_default();
            for binding in binds.as_slice() {
                base_binds.push(binding.clone());
            }
        }
        for (mode, bindings) in &overrider.mouse {
            let base_bindings = base.mouse.entry(*mode).or_default();
            for binding in bindings {
                base_bindings.push(*binding);
            }
        }
        base.binding_mode = overrider.binding_mode;
        base.rpc_enabled = overrider.rpc_enabled;
        base.rpc_port = overrider.rpc_port;
        base.rpc_allow_lan = overrider.rpc_allow_lan;
        base.trackpad_sensitivity = overrider.trackpad_sensitivity;
        base.page_borders = overrider.page_borders;
        base.dark_mode = overrider.dark_mode;
        base.invert_pdf = overrider.invert_pdf;
        base.open_sidebar = overrider.open_sidebar;
        base.default_search_method = overrider.default_search_method;
        base
    }

    pub fn dispatch(&mut self, e: iced::keyboard::Event) -> Option<&BindableMessage> {
        self.keyboard
            .get_mut(&self.binding_mode)
            .unwrap()
            .dispatch(e)
    }
}

/// Canonize a path so the printed source is absolute and free of `.`
/// components and symlinks. Falls back to the original path if it
/// cannot be resolved.
fn canonize_path(path: PathBuf) -> PathBuf {
    path.canonicalize().unwrap_or(path)
}

impl Default for Config {
    fn default() -> Self {
        // All default bindings apply to the Normal binding mode. The
        // Presentation mode starts out with its own set of bindings.
        let mut keyboard = HashMap::new();
        keyboard.insert(
            BindingMode::Normal,
            Keybinds::new(vec![
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
                // Arrow key movement (for non-vim users)
                Keybind::new(KeyInput::from_str("Up").unwrap(), BindableMessage::MoveUp),
                Keybind::new(
                    KeyInput::from_str("Down").unwrap(),
                    BindableMessage::MoveDown,
                ),
                Keybind::new(
                    KeyInput::from_str("Left").unwrap(),
                    BindableMessage::MoveLeft,
                ),
                Keybind::new(
                    KeyInput::from_str("Right").unwrap(),
                    BindableMessage::MoveRight,
                ),
                // Page navigation
                Keybind::new(
                    KeyInput::from_str("PageUp").unwrap(),
                    BindableMessage::PreviousPage,
                ),
                Keybind::new(
                    KeyInput::from_str("PageDown").unwrap(),
                    BindableMessage::NextPage,
                ),
                Keybind::new(
                    KeyInput::from_str("Ctrl+b").unwrap(),
                    BindableMessage::PageUp,
                ),
                Keybind::new(
                    KeyInput::from_str("Ctrl+f").unwrap(),
                    BindableMessage::PageDown,
                ),
                Keybind::new(
                    KeyInput::from_str("Ctrl+u").unwrap(),
                    BindableMessage::HalfPageUp,
                ),
                Keybind::new(
                    KeyInput::from_str("Ctrl+d").unwrap(),
                    BindableMessage::HalfPageDown,
                ),
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
                // Standard zoom controls
                Keybind::new(
                    KeyInput::from_str("Ctrl+0").unwrap(),
                    BindableMessage::ZoomHome,
                ),
                Keybind::new(
                    KeyInput::from_str("Ctrl+-").unwrap(),
                    BindableMessage::ZoomOut,
                ),
                Keybind::new(
                    KeyInput::from_str("Ctrl+Plus").unwrap(),
                    BindableMessage::ZoomIn,
                ),
                Keybind::new(
                    KeyInput::from_str("Ctrl+r").unwrap(),
                    BindableMessage::ToggleDarkModePdf,
                ),
                Keybind::new(
                    KeyInput::from_str("Ctrl+i").unwrap(),
                    BindableMessage::ToggleDarkModeUi,
                ),
                Keybind::new(
                    KeyInput::from_str("Ctrl+B").unwrap(),
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
                    KeyInput::from_str("F11").unwrap(),
                    BindableMessage::ToggleFullscreen,
                ),
                Keybind::new(
                    KeyInput::from_str("F10").unwrap(),
                    BindableMessage::TogglePresentationMode,
                ),
                Keybind::new(
                    KeyInput::from_str("F1").unwrap(),
                    BindableMessage::SinglePageLayout,
                ),
                Keybind::new(
                    KeyInput::from_str("F2").unwrap(),
                    BindableMessage::DoublePageLayout,
                ),
                Keybind::new(
                    KeyInput::from_str("F3").unwrap(),
                    BindableMessage::DoublePageTitlePageLayout,
                ),
                Keybind::new(
                    KeyInput::from_str("F4").unwrap(),
                    BindableMessage::PresentationLayout,
                ),
                Keybind::new(
                    KeyInput::from_str("[").unwrap(),
                    BindableMessage::RotatePageCounterClockwise,
                ),
                Keybind::new(
                    KeyInput::from_str("]").unwrap(),
                    BindableMessage::RotatePageClockwise,
                ),
                Keybind::new(
                    KeyInput::from_str("{").unwrap(),
                    BindableMessage::RotateAllPagesCounterClockwise,
                ),
                Keybind::new(
                    KeyInput::from_str("}").unwrap(),
                    BindableMessage::RotateAllPagesClockwise,
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
                Keybind::new(
                    KeyInput::from_str("Ctrl+w").unwrap(),
                    BindableMessage::CloseTab,
                ),
                // Search
                Keybind::new(
                    KeyInput::from_str("/").unwrap(),
                    BindableMessage::OpenSearch,
                ),
                Keybind::new(
                    KeyInput::from_str("Escape").unwrap(),
                    BindableMessage::CloseSearch,
                ),
                Keybind::new('n', BindableMessage::NextSearchResult),
                Keybind::new('p', BindableMessage::PreviousSearchResult),
                Keybind::new('N', BindableMessage::PreviousSearchResult),
                Keybind::new(
                    KeyInput::from_str("Ctrl+n").unwrap(),
                    BindableMessage::ToggleSearchMethod,
                ),
                // Tab navigation
                Keybind::new(KeyInput::from_str("Tab").unwrap(), BindableMessage::NextTab),
                Keybind::new(
                    KeyInput::from_str("Shift+Tab").unwrap(),
                    BindableMessage::PreviousTab,
                ),
            ]),
        );
        // The default Presentation mode bindings mirror the Presentation
        // section in assets/default.conf.
        keyboard.insert(
            BindingMode::Presentation,
            Keybinds::new(vec![
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
                    KeyInput::from_str("Space").unwrap(),
                    BindableMessage::NextPage,
                ),
                Keybind::new(
                    KeyInput::from_str("Shift+Space").unwrap(),
                    BindableMessage::PreviousPage,
                ),
                Keybind::new(
                    KeyInput::from_str("Up").unwrap(),
                    BindableMessage::PreviousPage,
                ),
                Keybind::new(
                    KeyInput::from_str("Down").unwrap(),
                    BindableMessage::NextPage,
                ),
                Keybind::new(
                    KeyInput::from_str("Left").unwrap(),
                    BindableMessage::PreviousPage,
                ),
                Keybind::new(
                    KeyInput::from_str("Right").unwrap(),
                    BindableMessage::NextPage,
                ),
                Keybind::new(
                    KeyInput::from_str("Ctrl+r").unwrap(),
                    BindableMessage::ToggleDarkModePdf,
                ),
                Keybind::new(
                    KeyInput::from_str("Ctrl+k").unwrap(),
                    BindableMessage::TogglePageBorders,
                ),
                Keybind::new(
                    KeyInput::from_str("F11").unwrap(),
                    BindableMessage::ToggleFullscreen,
                ),
                Keybind::new(
                    KeyInput::from_str("F10").unwrap(),
                    BindableMessage::TogglePresentationMode,
                ),
                Keybind::new(
                    KeyInput::from_str("Escape").unwrap(),
                    BindableMessage::TogglePresentationMode,
                ),
                Keybind::new(KeyInput::from_str("_").unwrap(), BindableMessage::ZoomFit),
                Keybind::new(KeyInput::from_str("q").unwrap(), BindableMessage::Exit),
            ]),
        );

        let mut mouse = HashMap::new();
        mouse.insert(
            BindingMode::Normal,
            vec![
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
        );
        // The default Presentation mode mouse bindings mirror the
        // Presentation section in assets/default.conf.
        mouse.insert(
            BindingMode::Presentation,
            vec![
                (
                    MouseInput {
                        button: MouseButton::Left,
                        modifiers: MouseModifiers {
                            ctrl: false,
                            shift: false,
                        },
                    },
                    MouseAction::NextPage,
                ),
                (
                    MouseInput {
                        button: MouseButton::Left,
                        modifiers: MouseModifiers {
                            ctrl: false,
                            shift: true,
                        },
                    },
                    MouseAction::PreviousPage,
                ),
            ],
        );

        Config {
            binding_mode: BindingMode::Normal,
            keyboard,
            mouse,
            rpc_enabled: false,
            rpc_port: 7890,
            rpc_allow_lan: false,
            trackpad_sensitivity: 1.0,
            page_borders: true,
            dark_mode: true,
            invert_pdf: false,
            open_sidebar: false,
            default_search_method: SearchMethod::PlainText,
            open_fullscreen_default: false,
            open_presentation_default: false,
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
    use keybinds2::{KeyInput, Keybind};

    use super::*;

    /// Strip ANSI escape sequences so assertions on formatted output work
    /// the same whether or not `colored` emits color codes (colors are
    /// active in a TTY but auto-disabled when piping output).
    fn strip_ansi(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        let mut chars = s.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\x1b' {
                // Skip everything up to and including the terminator
                // (ESC[ ... m in practice, but handle any final byte).
                if chars.peek() == Some(&'[') {
                    chars.next();
                    for c in chars.by_ref() {
                        if c.is_ascii_alphabetic() {
                            break;
                        }
                    }
                }
            } else {
                out.push(c);
            }
        }
        out
    }

    #[test]
    pub fn can_parse_vim_bindings() {
        let _config = Config {
            keyboard: HashMap::from([(
                BindingMode::Normal,
                Keybinds::new(vec![
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
            )]),
            mouse: HashMap::from([(BindingMode::Normal, Vec::new())]),
            rpc_enabled: false,
            rpc_port: 7890,
            rpc_allow_lan: false,
            trackpad_sensitivity: 1.0,
            page_borders: true,
            dark_mode: true,
            invert_pdf: false,
            open_sidebar: false,
            default_search_method: SearchMethod::PlainText,
            ..Default::default()
        };
    }

    #[test]
    pub fn can_parse_config_file() {
        let contents = include_str!("../assets/default.conf");
        let config = Config::from_str(contents).unwrap();
        let default_cfg = Config::default();

        // Check that parsed and default configs have the exact same modes
        let mut modes = config.keyboard.keys().collect::<Vec<_>>();
        let mut default_modes = default_cfg.keyboard.keys().collect::<Vec<_>>();
        modes.sort();
        default_modes.sort();
        assert_eq!(modes, default_modes);

        let mut modes = config.mouse.keys().collect::<Vec<_>>();
        let mut default_modes = default_cfg.mouse.keys().collect::<Vec<_>>();
        modes.sort();
        default_modes.sort();
        assert_eq!(modes, default_modes);

        // Check keyboard bindings for each mode
        for (mode, default_binds) in &default_cfg.keyboard {
            let binds = config.keyboard[mode].as_slice();
            let default_binds = default_binds.as_slice();
            assert_eq!(binds.len(), default_binds.len());
            for (b1, b2) in binds.iter().zip(default_binds) {
                assert_eq!(b1.seq, b2.seq, "in mode {mode:?}");
                assert_eq!(b1.action, b2.action, "in mode {mode:?}");
            }
        }

        // Check mouse bindings for each mode
        for (mode, default_binds) in &default_cfg.mouse {
            let binds = &config.mouse[mode];
            assert_eq!(binds.len(), default_binds.len());
            for (b1, b2) in binds.iter().zip(default_binds.iter()) {
                assert_eq!(b1.0, b2.0, "in mode {mode:?}"); // MouseInput
                assert_eq!(b1.1, b2.1, "in mode {mode:?}"); // MouseAction
            }
        }

        // Check other settings
        assert_eq!(config.rpc_enabled, default_cfg.rpc_enabled);
        assert_eq!(config.rpc_port, default_cfg.rpc_port);
        assert_eq!(config.rpc_allow_lan, default_cfg.rpc_allow_lan);
        assert_eq!(
            config.trackpad_sensitivity,
            default_cfg.trackpad_sensitivity
        );
        assert_eq!(config.page_borders, default_cfg.page_borders);
        assert_eq!(config.dark_mode, default_cfg.dark_mode);
        assert_eq!(config.invert_pdf, default_cfg.invert_pdf);
        assert_eq!(config.open_sidebar, default_cfg.open_sidebar);
        assert_eq!(
            config.default_search_method,
            default_cfg.default_search_method
        );
    }

    #[allow(clippy::bool_assert_comparison)]
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
        assert!(result.errors[0]
            .message
            .contains("Unknown command: UnknownCommand"));
    }

    #[test]
    pub fn error_handling_invalid_bind_args() {
        let config_str = "Bind j";
        let result = Config::parse_with_errors(config_str);

        assert!(result.has_errors());
        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.errors[0].line_number, 1);
        assert!(result.errors[0]
            .message
            .contains("Bind command requires 3 arguments"));
    }

    #[test]
    pub fn error_handling_invalid_action() {
        let config_str = "Bind j InvalidAction";
        let result = Config::parse_with_errors(config_str);

        assert!(result.has_errors());
        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.errors[0].line_number, 1);
        assert!(result.errors[0]
            .message
            .contains("Unknown action: InvalidAction"));
    }

    #[test]
    pub fn error_handling_invalid_mouse_input() {
        let config_str = "MouseBind InvalidMouse Panning";
        let result = Config::parse_with_errors(config_str);

        assert!(result.has_errors());
        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.errors[0].line_number, 1);
        assert!(result.errors[0]
            .message
            .contains("Invalid mouse input 'InvalidMouse'"));
    }

    #[test]
    pub fn error_handling_invalid_set_value() {
        let config_str = "Set RpcPort invalid_port";
        let result = Config::parse_with_errors(config_str);

        assert!(result.has_errors());
        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.errors[0].line_number, 1);
        assert!(result.errors[0]
            .message
            .contains("Invalid port number: 'invalid_port'"));
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
        assert!(!result.config.keyboard[&BindingMode::Normal]
            .as_slice()
            .is_empty());
    }

    #[test]
    pub fn error_handling_unterminated_quotes() {
        let config_str = r#"Bind "unterminated quote MoveUp"#;
        let result = Config::parse_with_errors(config_str);

        assert!(result.has_errors());
        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.errors[0].line_number, 1);
        assert!(result.errors[0]
            .message
            .contains("Unterminated quoted string"));
    }

    #[test]
    pub fn warn_on_implicit_normal_mode() {
        // The old config format omits the mode; each offending line should
        // emit one warning while still binding to Normal mode.
        let config_str = r#"
Bind j MoveDown
Bind k Normal MoveUp
MouseBind MouseLeft Panning
MouseBind MouseRight Normal Selection
"#;
        let result = Config::parse_with_errors(config_str);

        assert!(!result.has_errors());
        assert_eq!(result.warnings.len(), 2);

        // One warning per offending line, with correct line numbers
        assert_eq!(result.warnings[0].line_number, 2);
        assert!(result.warnings[0].is_warning);
        assert!(result.warnings[0].message.contains("Deprecated"));
        assert_eq!(result.warnings[1].line_number, 4);
        assert!(result.warnings[1].is_warning);
        assert!(result.warnings[1].message.contains("MouseBind"));

        // All bindings are still applied to Normal mode
        let binds = result.config.keyboard[&BindingMode::Normal].as_slice();
        assert_eq!(binds.len(), 2);
        assert_eq!(binds[0].action, BindableMessage::MoveDown);
        assert_eq!(binds[1].action, BindableMessage::MoveUp);
        let mouse_binds = &result.config.mouse[&BindingMode::Normal];
        assert_eq!(mouse_binds.len(), 2);
        assert_eq!(mouse_binds[0].1, MouseAction::Panning);
        assert_eq!(mouse_binds[1].1, MouseAction::Selection);

        // The formatted output lists each warning on its own line
        let formatted = strip_ansi(&result.format_warnings());
        assert!(formatted.contains("Configuration parsing warnings:"));
        assert!(formatted.contains("Line 2:"));
        assert!(formatted.contains("Line 4:"));

        // With a known source file, the header names it
        let mut result = Config::parse_with_errors(config_str);
        result.set_source(PathBuf::from("/home/user/.config/miro-pdf/miro.conf"));
        assert!(strip_ansi(&result.format_warnings())
            .contains("Configuration parsing warnings in /home/user/.config/miro-pdf/miro.conf:"));
    }

    #[test]
    pub fn errors_and_warnings_name_source_file() {
        let config_str = "UnknownCommand arg1";
        let mut result = Config::parse_with_errors(config_str);
        result.set_source(PathBuf::from("/home/user/miro.conf"));

        let formatted = strip_ansi(&result.format_errors());
        assert!(
            formatted.contains("Configuration parsing errors in /home/user/miro.conf:"),
            "got: {formatted}"
        );
        assert!(formatted.contains("Line 1:"));
    }

    #[test]
    pub fn no_warnings_for_explicit_modes() {
        let result = Config::parse_with_errors(include_str!("../assets/default.conf"));

        assert!(!result.has_errors());
        assert!(!result.has_warnings());
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
        assert_eq!(
            result.config.keyboard[&BindingMode::Normal]
                .as_slice()
                .len(),
            2
        );
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
        let formatted = strip_ansi(&result.format_errors());
        assert!(formatted.contains("Configuration parsing errors:"));
        assert!(formatted.contains("Line 2:"));
        assert!(formatted.contains("Line 3:"));
        assert!(formatted.contains("Line 4:"));

        // Re-enable colors
        control::unset_override();
    }

    #[test]
    pub fn implicit_normal_mode_backwards_compatibility() {
        // Bindings without an explicit mode should be treated as Normal mode
        // bindings, for backwards compatibility with the old config format.
        let config_str = r#"
Bind j MoveDown
Bind Tab NextTab
MouseBind MouseLeft Panning
MouseBind Ctrl+ScrollUp ZoomIn
"#;
        let result = Config::parse_with_errors(config_str);

        assert!(!result.has_errors());

        // Both modes must exist, only Normal should have bindings
        assert_eq!(result.config.keyboard.len(), 2);
        assert_eq!(result.config.mouse.len(), 2);
        assert_eq!(
            result.config.keyboard[&BindingMode::Presentation]
                .as_slice()
                .len(),
            0
        );
        assert_eq!(result.config.mouse[&BindingMode::Presentation].len(), 0);

        // The binds ended up in Normal mode
        let binds = result.config.keyboard[&BindingMode::Normal].as_slice();
        assert_eq!(binds.len(), 2);
        assert_eq!(binds[0].action, BindableMessage::MoveDown);
        assert_eq!(binds[1].action, BindableMessage::NextTab);

        let mouse_binds = &result.config.mouse[&BindingMode::Normal];
        assert_eq!(mouse_binds.len(), 2);
        assert_eq!(mouse_binds[0].1, MouseAction::Panning);
        assert_eq!(mouse_binds[1].1, MouseAction::ZoomIn);

        // The binds are active in the default (Normal) binding mode
        let mut config = result.config;
        assert_eq!(
            config.get_mouse_action(MouseInput {
                button: MouseButton::Left,
                modifiers: MouseModifiers::default()
            }),
            Some(MouseAction::Panning)
        );
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
        assert!(result.errors[0]
            .message
            .contains("Invalid float value for TrackpadSensitivity"));
    }

    #[test]
    pub fn can_parse_default_search_method() {
        let config_str = "Set DefaultSearchMethod Regex";
        let result = Config::parse_with_errors(config_str);

        assert!(!result.has_errors());
        assert_eq!(result.config.default_search_method, SearchMethod::Regex);
    }

    #[test]
    pub fn error_handling_invalid_default_search_method() {
        let config_str = "Set DefaultSearchMethod InvalidMethod";
        let result = Config::parse_with_errors(config_str);

        assert!(result.has_errors());
        assert_eq!(result.errors.len(), 1);
        assert!(result.errors[0]
            .message
            .contains("Unknown search method: 'InvalidMethod'"));
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
            assert!(!result.config.keyboard[&BindingMode::Normal]
                .as_slice()
                .is_empty());
        }
    }
}
