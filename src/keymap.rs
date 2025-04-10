use std::{collections::HashMap, str::FromStr};

use iced::{
    advanced::graphics::core::SmolStr,
    keyboard::{self, Key, Modifiers},
};
use serde::{Deserialize, Serialize};

use crate::{app::AppMessage, pdf::PdfMessage};

const MOVE_STEP: f32 = 40.0;

pub struct KeyMap<Message>
where
    for<'de> Message: Clone + Serialize + Deserialize<'de>,
{
    bindings: HashMap<(Key, Modifiers), Message>,
}

impl<Message> KeyMap<Message>
where
    for<'de> Message: Clone + Serialize + Deserialize<'de>,
{
    pub fn event(&self, key: Key, modifiers: Modifiers) -> Option<Message> {
        for (x, msg) in self.bindings.iter() {
            let (k, m) = x;
            if *k == key && *m == modifiers {
                return Some(msg.clone());
            }
        }
        None
    }

    pub fn add_char_binding(&mut self, key: &str, modifiers: Modifiers, message: Message) {
        self.bindings.insert(
            (Key::Character(SmolStr::from_str(key).unwrap()), modifiers),
            message,
        );
    }
}

impl Default for KeyMap<AppMessage> {
    fn default() -> Self {
        let mut out = Self {
            bindings: HashMap::new(),
        };

        out.add_char_binding(
            "k",
            Modifiers::SHIFT,
            AppMessage::PdfMessage(PdfMessage::PreviousPage),
        );
        out.add_char_binding(
            "k",
            Modifiers::empty(),
            AppMessage::PdfMessage(PdfMessage::MoveVertical(-MOVE_STEP)),
        );
        out.add_char_binding(
            "j",
            Modifiers::SHIFT,
            AppMessage::PdfMessage(PdfMessage::NextPage),
        );
        out.add_char_binding(
            "j",
            Modifiers::empty(),
            AppMessage::PdfMessage(PdfMessage::MoveVertical(MOVE_STEP)),
        );
        out.add_char_binding(
            "+",
            Modifiers::empty(),
            AppMessage::PdfMessage(PdfMessage::ZoomIn),
        );
        out.add_char_binding(
            "-",
            Modifiers::empty(),
            AppMessage::PdfMessage(PdfMessage::ZoomOut),
        );
        out.add_char_binding(
            "0",
            Modifiers::empty(),
            AppMessage::PdfMessage(PdfMessage::ZoomHome),
        );
        out.add_char_binding(
            "-",
            Modifiers::SHIFT,
            AppMessage::PdfMessage(PdfMessage::ZoomFit),
        );
        out.add_char_binding(
            "h",
            Modifiers::empty(),
            AppMessage::PdfMessage(PdfMessage::MoveHorizontal(-MOVE_STEP)),
        );
        out.add_char_binding("h", Modifiers::SHIFT, AppMessage::PreviousTab);
        out.add_char_binding(
            "l",
            Modifiers::empty(),
            AppMessage::PdfMessage(PdfMessage::MoveHorizontal(MOVE_STEP)),
        );
        out.add_char_binding("l", Modifiers::SHIFT, AppMessage::NextTab);

        out
    }
}
