use crate::{
    geometry::{Rect, Vector},
    config::MouseAction,
};
use serde::{Deserialize, Serialize};
use strum::EnumString;

mod inner;
pub mod link_extraction;
pub mod outline_extraction;
pub mod text_extraction;
pub mod widget;

#[derive(Debug, Clone, Serialize, Deserialize, EnumString, Default)]
pub enum PdfMessage {
    NextPage,
    PreviousPage,
    SetPage(i32),
    SetTranslation(Vector<f32>),
    ZoomIn,
    ZoomOut,
    ZoomHome,
    ZoomFit,
    Move(Vector<f32>),
    UpdateBounds(Rect<f32>),
    MouseMoved(Vector<f32>),
    MouseLeftDown(bool),            // bool indicates if Shift is pressed
    MouseLeftUp(bool),              // bool indicates if Shift is pressed
    MouseAction(MouseAction, bool), // action and whether it's pressed (true) or released (false)
    ToggleLinkHitboxes,
    ActivateLink(usize), // Activate link by index
    CloseLinkHitboxes,   // Close/hide link hitboxes
    FileChanged,
    ReallocPixmap,
    PrintPdf,
    // Layout controls
    ToggleTwoPage,
    ToggleCoverPage,
    #[default]
    None,
}
