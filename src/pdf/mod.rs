use std::sync::atomic::{AtomicBool, Ordering};

use crate::{
    app::AppMessage,
    config::MouseAction,
    geometry::{Rect, Vector},
    pdf::page_layout::PageLayoutKind,
};
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
///
/// Scanning is chunked page by page so the `cancel` flag can be checked between chunks;
/// a superseded scan therefore exits within milliseconds instead of running to completion.
/// This matters because a task can never be cancelled once it is running on the blocking
/// thread pool, and iced's tokio runtime waits for blocking tasks on shutdown.
pub fn find_search_matches(
    haystack: &str,
    needle: &str,
    method: SearchMethod,
    char_bboxes: &[(usize, usize, Rect<f32>)],
    cancel: &AtomicBool,
) -> Vec<SearchMatch> {
    if needle.is_empty() {
        return vec![];
    }
    if cancel.load(Ordering::Relaxed) {
        return vec![];
    }

    // Page boundaries in `haystack`, derived from the first character offset of every page.
    // `char_bboxes` is built in document order, so the offsets are monotonically increasing.
    let mut page_starts: Vec<usize> = vec![];
    let mut last_page = usize::MAX;
    for &(page_idx, byte_offset, _) in char_bboxes {
        if page_idx != last_page {
            page_starts.push(byte_offset);
            last_page = page_idx;
        }
    }
    if page_starts.is_empty() {
        page_starts.push(0);
    }
    let page_ends: Vec<usize> = page_starts
        .iter()
        .skip(1)
        .copied()
        .chain(std::iter::once(haystack.len()))
        .collect();

    // Overlap between consecutive chunks: a match that spans a page boundary must be fully
    // contained in at least one chunk's scan region. Plain text matches are exactly
    // `needle.len()` long. Regex matches can be longer than the pattern; allow some slack so
    // only truly pathological cross-page matches (longer than the overlap window) would be
    // missed.
    let overlap = needle.len() + 4096;

    let mut byte_ranges: Vec<(usize, usize)> = vec![];
    match method {
        SearchMethod::PlainText => {
            let _span = tracy_client::span!("Plain text search");
            for (chunk_idx, &chunk_end) in page_ends.iter().enumerate() {
                if cancel.load(Ordering::Relaxed) {
                    return vec![];
                }
                let scan_start =
                    floor_char_boundary(haystack, page_starts[chunk_idx].saturating_sub(overlap));
                for (start, matched) in haystack[scan_start..chunk_end].match_indices(needle) {
                    byte_ranges.push((scan_start + start, scan_start + start + matched.len()));
                }
            }
        }
        SearchMethod::Regex => {
            let _span = tracy_client::span!("Regex search");
            if let Ok(re) = regex::Regex::new(needle) {
                for (chunk_idx, &chunk_end) in page_ends.iter().enumerate() {
                    if cancel.load(Ordering::Relaxed) {
                        return vec![];
                    }
                    let scan_start = floor_char_boundary(
                        haystack,
                        page_starts[chunk_idx].saturating_sub(overlap),
                    );
                    for capture in re.captures_iter(&haystack[scan_start..chunk_end]) {
                        let m = capture.get_match();
                        byte_ranges.push((scan_start + m.start(), scan_start + m.end()));
                    }
                }
            }
        }
    }
    // Overlapping chunks can find the same match twice.
    byte_ranges.sort_unstable();
    byte_ranges.dedup();

    // Map match byte ranges to bounding boxes in a single merge pass. Both `byte_ranges`
    // (sorted, non-overlapping) and `char_bboxes` (monotonically increasing offsets) are
    // ordered, so one shared cursor suffices. The previous nested loop re-scanned the entire
    // `char_bboxes` array once PER MATCH, which is quadratic in the document size.
    let mut matches = vec![];
    let mut cursor = 0;
    for (start, end) in byte_ranges {
        if cancel.load(Ordering::Relaxed) {
            return vec![];
        }
        while cursor < char_bboxes.len() && char_bboxes[cursor].1 < start {
            cursor += 1;
        }
        let mut char_rects = vec![];
        while cursor < char_bboxes.len() && char_bboxes[cursor].1 < end {
            char_rects.push((char_bboxes[cursor].0, char_bboxes[cursor].2));
            cursor += 1;
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

/// Largest index ≤ `idx` that is a char boundary of `haystack`.
fn floor_char_boundary(haystack: &str, mut idx: usize) -> usize {
    while idx > 0 && !haystack.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
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
    SetLayout(PageLayoutKind),
    /// Leave overview mode and restore the layout that was active before it was
    /// opened. When `true`, also navigate to the selected overview page.
    ExitOverview(bool),
    RotatePageClockwise,
    RotatePageCounterClockwise,
    RotateAllPagesClockwise,
    RotateAllPagesCounterClockwise,
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
