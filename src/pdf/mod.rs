use std::path::PathBuf;

use crate::{custom_serde_functions::*, geometry::Rect};
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
    #[default]
    None,
}
