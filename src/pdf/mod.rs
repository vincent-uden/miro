use crate::{app::AppMessage, config::MouseAction, geometry::{Rect, Vector}, pdf::page_layout::PageLayout};
use serde::{Deserialize, Serialize};
use strum::EnumString;

pub mod page_layout;
pub mod widget;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, EnumString, Default, PartialEq, Eq)]
pub enum SearchMethod {
    #[default]
    PlainText,
    Regex,
}

/// A single search result, potentially spanning multiple pages.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchMatch {
    pub start_byte: usize,
    pub end_byte: usize,
    pub pages: std::ops::Range<usize>,
    /// Merged bounding boxes per line: (page_index, bounding_box).
    pub rects: Vec<(usize, Rect<f32>)>,
}

/// Find all search matches in `haystack` and map them to bounding boxes.
pub fn find_search_matches(
    haystack: &str,
    needle: &str,
    method: SearchMethod,
    char_bboxes: &[(usize, usize, Rect<f32>)],
) -> Vec<SearchMatch> {
    if needle.is_empty() {
        return vec![];
    }

    // Collect (start_byte, end_byte) for each match.
    let mut byte_ranges = vec![];
    match method {
        SearchMethod::PlainText => {
            let _span = tracy_client::span!("Plain text search");
            for (start, matched) in haystack.match_indices(needle) {
                byte_ranges.push((start, start + matched.len()));
            }
        }
        SearchMethod::Regex => {
            let _span = tracy_client::span!("Regex search");
            if let Ok(re) = regex::Regex::new(needle) {
                for capture in re.captures_iter(haystack) {
                    let m = capture.get_match();
                    byte_ranges.push((m.start(), m.end()));
                }
            }
        }
    }

    let mut matches = vec![];
    for (start, end) in byte_ranges {
        let mut char_rects = vec![];
        for &(page_idx, byte_offset, rect) in char_bboxes {
            if byte_offset >= start && byte_offset < end {
                char_rects.push((page_idx, rect));
            }
        }
        let rects = merge_search_rects(&char_rects);
        if !rects.is_empty() {
            let first_page = rects[0].0;
            let last_page = rects[rects.len() - 1].0;
            matches.push(SearchMatch {
                start_byte: start,
                end_byte: end,
                pages: first_page..last_page + 1,
                rects,
            });
        }
    }
    matches
}

/// Merge consecutive character bounding boxes that are on the same page and
/// vertically overlap (i.e., belong to the same line).
pub fn merge_search_rects(char_rects: &[(usize, Rect<f32>)]) -> Vec<(usize, Rect<f32>)> {
    if char_rects.is_empty() {
        return vec![];
    }
    let mut merged = vec![];
    let mut current_page = char_rects[0].0;
    let mut current = char_rects[0].1;
    for &(page_idx, rect) in &char_rects[1..] {
        // Vertically overlapping means same line.
        let same_line =
            page_idx == current_page && rect.x0.y < current.x1.y && rect.x1.y > current.x0.y;
        if same_line {
            current.x0.x = current.x0.x.min(rect.x0.x);
            current.x0.y = current.x0.y.min(rect.x0.y);
            current.x1.x = current.x1.x.max(rect.x1.x);
            current.x1.y = current.x1.y.max(rect.x1.y);
        } else {
            merged.push((current_page, current));
            current_page = page_idx;
            current = rect;
        }
    }
    merged.push((current_page, current));
    merged
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
    NextSearchResult,
    PreviousSearchResult,
    UpdateSearchNeedle(String),
    SetSearchMethod(SearchMethod),
    ToggleSearchMethod,
    /// Show comment popup for the given comment index
    ShowComment(usize),
    /// Close the comment popup
    CloseComment,
    #[strum(disabled)]
    #[serde(skip)]
    SearchResultsReady(Vec<SearchMatch>, u64),
    #[default]
    None,
}

impl From<PdfMessage> for AppMessage {
    fn from(value: PdfMessage) -> Self {
        AppMessage::PdfMessage(value)
    }
}
