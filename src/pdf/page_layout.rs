use mupdf::{Document, Page};

use crate::geometry::{Rect, Vector};

#[derive(Debug, Clone, Copy)]
pub enum PageLayout {
    /// One page per row, many rows
    SinglePage,
    /// Two pages per row, many rows
    TwoPage,
    /// Two pages per row, many rows, except for the first page which is on its own
    TwoPageTitlePage,
    /// Only one page on the screen at a time
    Presentation,
}

impl PageLayout {
    /// Returns visible pages and their bounding boxes relative to the widgets origin.
    fn pages_rects(
        &self,
        doc: &Document,
        translation: Vector<f32>,
        scale: f32,
        fractional_scale: f64,
        viewport: Rect<f32>,
    ) -> Vec<(Page, Rect<f32>)> {
        todo!("");
    }

    /// Returns the translation that would leave the page at [page_idx] visible on the screen. If
    /// `page_idx > doc.page_count()` this will move to the last page.
    fn translation_for_page(
        &self,
        doc: &Document,
        scale: f32,
        fractional_scale: f64,
        page_idx: usize,
        viewport: Rect<f32>,
    ) -> Vector<f32> {
        todo!("")
    }

    /// Returns the height of the row of pages occupying the middle of the creen
    fn page_set_height(
        &self,
        doc: &Document,
        translation: Vector<f32>,
        scale: f32,
        fractional_scale: f64,
        viewport: Rect<f32>,
    ) -> f32 {
        todo!("")
    }
}
