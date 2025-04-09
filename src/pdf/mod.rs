use std::path::PathBuf;

use crate::custom_serde_functions::*;
use iced::Rectangle;
use serde::{Deserialize, Serialize};

mod inner;
pub mod widget;

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    #[serde(
        serialize_with = "serialize_rectangle",
        deserialize_with = "deserialize_rectangle"
    )]
    UpdateBounds(Rectangle),
}
