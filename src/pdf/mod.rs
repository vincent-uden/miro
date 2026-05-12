use crate::{
    config::MouseAction,
    geometry::{Vector},
    pdf::page_layout::PageLayout,
};
use serde::{Deserialize, Serialize};
use strum::EnumString;

pub mod page_layout;
pub mod widget;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, EnumString, Default)]
pub enum SearchMethod {
    #[default]
    PlainText,
    Regex,
    FuzzyFinding,
}

#[derive(Debug, Clone, Serialize, Deserialize, EnumString, Default)]
pub enum PdfMessage {
    NextPage,
    PreviousPage,
    PageUp,
    PageDown,
    HalfPageUp,
    HalfPageDown,
    SetPage(usize),
    SetTranslation(Vector<f32>),
    /// Translation and scale
    SetLocation(Vector<f32>, f32),
    SetLayout(PageLayout),
    ZoomIn,
    ZoomOut,
    ZoomHome,
    ZoomFit,
    /// Move some distance in Document space
    Move(Vector<f32>),
    MouseMoved(Vector<f32>),
    /// A [MouseAction] and whether it's pressed (true) or released (false)
    MouseAction(MouseAction, bool),
    ToggleLinkHitboxes,
    /// Activate link by index
    ActivateLink(usize),
    /// Close/hide link hitboxes
    CloseLinkHitboxes,
    FileChanged,
    PrintPdf,
    HighlightSearchResults,
    HideSearchResults,
    JumpToSearchResult(usize),
    UpdateSearchNeedle(String),
    SetSearchMethod(SearchMethod),
    #[default]
    None,
}
