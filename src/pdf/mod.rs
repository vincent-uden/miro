use std::path::PathBuf;

use crate::geometry::{Rect, Vector};
use serde::{Deserialize, Serialize};
use strum::EnumString;
use worker::WorkerResponse;

mod inner;
pub mod widget;
pub mod worker;

pub use text_extraction::*;
mod text_extraction;

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
    #[strum(disabled)]
    #[serde(skip)]
    WorkerResponse(WorkerResponse),
    #[default]
    None,
}
