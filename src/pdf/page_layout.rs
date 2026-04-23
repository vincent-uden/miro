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
    const GAP: f32 = 10.0;

    /// Returns visible pages and their bounding boxes relative to the widgets origin. A translation
    /// of (0,0) should result in the first page row being centered on the screen. Scale is applied
    /// after translation with respect to the center of the screen. Thus zooming doesn't move the
    /// doucment.
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
                let mut pos = Vector::zero();
                for page in pages.flatten() {
                    let mut bounds = page.bounds()?;
                    bounds.y0 += pos.y;
                    bounds.y1 += pos.y;
                    pos += bounds.size().into();
                    pos.y += Self::GAP;
                    out.push(bounds.into());
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
