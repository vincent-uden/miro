use std::path::PathBuf;

use crate::geometry::{Rect, Vector};
use serde::{Deserialize, Serialize};
use strum::EnumString;
use worker::WorkerResponse;

mod inner;
pub mod widget;
pub mod worker;


mod text_extraction;
mod link_extraction;
pub mod outline_extraction;

#[derive(Debug, Clone, Serialize, Deserialize, EnumString, Default)]
pub enum PdfMessage {
    OpenFile(PathBuf),
    NextPage,
    PreviousPage,
    SetPage(i32),
    ZoomIn,
    ZoomOut,
    ZoomHome,
    ZoomFit,
    MoveHorizontal(f32),
    MoveVertical(f32),
    UpdateBounds(Rect<f32>),
    MouseMoved(Vector<f32>),
    MouseLeftDown(bool), // bool indicates if Ctrl is pressed
    MouseRightDown,
    MouseLeftUp(bool), // bool indicates if Ctrl is pressed
    MouseRightUp,
    ToggleLinkHitboxes,
    #[strum(disabled)]
    #[serde(skip)]
    WorkerResponse(WorkerResponse),
    #[default]
    None,
}
