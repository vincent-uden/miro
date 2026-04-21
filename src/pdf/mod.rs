use crate::{
    geometry::{Rect, Vector},
    config::MouseAction,
};
use serde::{Deserialize, Serialize};
use strum::EnumString;

pub mod link_extraction;
pub mod outline_extraction;
pub mod text_extraction;
pub mod widget;

#[derive(Debug, Clone, Serialize, Deserialize, EnumString, Default)]
pub enum PdfMessage {
    PageDown,
    PageUp,
    SetPage(usize),
    SetTranslation(Vector<f32>),
    ZoomIn,
    ZoomOut,
    ZoomHome,
    ZoomFit,
    Move(Vector<f32>),
    MouseMoved(Vector<f32>),
    /// bool indicates if Shift is pressed
    MouseLeftDown(bool),
    /// bool indicates if Shift is pressed
    MouseLeftUp(bool),
    /// A [MouseAction] and whether it's pressed (true) or released (false)
    MouseAction(MouseAction, bool),
    ToggleLinkHitboxes,
    /// Activate link by index
    ActivateLink(usize),
    /// Close/hide link hitboxes
    CloseLinkHitboxes,
    FileChanged,
    PrintPdf,
    #[default]
    None,
}
