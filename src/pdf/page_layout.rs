use anyhow::Result;
use iced::Size;
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
    pub fn pages_rects(
        &self,
        doc: &Document,
        translation: Vector<f32>,
        scale: f32,
        fractional_scale: f32,
        viewport: Size<f32>,
    ) -> Result<Vec<Rect<f32>>> {
        let mut out = vec![];
        let mut pages = doc.pages()?;
        match self {
            PageLayout::SinglePage => {
                for page in pages.flatten() {
                    out.push(page.bounds()?.into());
                }
            }
            PageLayout::TwoPage => todo!(),
            PageLayout::TwoPageTitlePage => todo!(),
            PageLayout::Presentation => todo!(),
        }
        Ok(out)
    }

    /// Returns the translation that would leave the page at [page_idx] visible on the screen. If
    /// `page_idx > doc.page_count()` this will move to the last page.
    pub fn translation_for_page(
        &self,
        doc: &Document,
        scale: f32,
        fractional_scale: f32,
        page_idx: usize,
        viewport: Rect<f32>,
    ) -> Vector<f32> {
        todo!("")
    }

    /// Returns the height of the row of pages occupying the middle of the creen
    pub fn page_set_height(
        &self,
        doc: &Document,
        translation: Vector<f32>,
        scale: f32,
        fractional_scale: f32,
        viewport: Rect<f32>,
    ) -> f32 {
        todo!("")
    }
}
