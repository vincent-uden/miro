use std::path::PathBuf;

use crate::geometry::{Rect, Vector};
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

mod inner;
pub mod widget;

#[derive(Debug, Clone, Serialize, Deserialize, EnumString, Default)]
pub enum PdfMessage {
    OpenFile(PathBuf),
    RefreshFile,
    NextPage,
    PreviousPage,
    ZoomIn,
    ZoomOut,
    ZoomHome,
    ZoomFit,
    MoveHorizontal(f32),
    MoveVertical(f32),
    UpdateBounds(Rect<f32>),
    MouseMoved(Vector<f32>),
    MouseLeftDown,
    MouseRightDown,
    MouseLeftUp,
    MouseRightUp,
    #[default]
    None,
}
