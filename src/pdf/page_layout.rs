use anyhow::{anyhow, Result};
use iced::Size;
use mupdf::Document;
use num::Integer;
use serde::{Deserialize, Serialize};
use strum::EnumString;

use crate::geometry::{Rect, Vector};

#[derive(Debug, Clone, Serialize, Deserialize, EnumString, Default)]
pub enum PageLayout {
    #[default]
    /// One page per row, many rows
    SinglePage,
    /// Two pages per row, many rows
    DoublePage,
    /// Two pages per row, many rows, except for the first page which is on its own
    DoublePageTitlePage,
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
        mut pages: mupdf::document::PageIter<'_>,
        translation: Vector<f32>, // In document space
        scale: f32,
        fractional_scale: f32,
        viewport: Size<f32>,
    ) -> Result<Vec<Rect<f32>>> {
        let mut out: Vec<Rect<f32>> = vec![];
        let vsize: Vector<_> = viewport.into();
        let effective_scale = scale * fractional_scale;
        match self {
            PageLayout::SinglePage => {
                let mut pos: Vector<f32> = Vector::zero();
                let mut prev_bounds = Rect::default();
                for (i, page) in pages.flatten().enumerate() {
                    let mut bounds: Rect<f32> = page.bounds()?.into();
                    bounds.translate((vsize - bounds.size()).scaled(0.5));
                    bounds.translate(translation.scaled(effective_scale));
                    bounds = bounds.scaled(effective_scale);
                    if i != 0 {
                        pos.y += (prev_bounds.height() + bounds.height()) / 2.0;
                    }
                    bounds.translate(pos);

                    pos.y += Self::GAP * effective_scale;
                    prev_bounds = bounds;

                    out.push(bounds);
                }
            }
            PageLayout::DoublePage => {
                let mut pos: Vector<f32> = Vector::zero();
                for (i, page) in pages.flatten().enumerate() {
                    let mut bounds: Rect<f32> = page.bounds()?.into();
                    bounds.translate(pos);
                    bounds.translate((vsize - bounds.size()).scaled(0.5));
                    bounds.translate(translation.scaled(effective_scale));
                    bounds = bounds.scaled(effective_scale);

                    if i.is_odd() {
                        pos.y += bounds.size().y;
                        pos.y += Self::GAP * effective_scale;
                        pos.x = 0.0;
                    } else {
                        pos.x += bounds.size().x;
                        pos.x += Self::GAP * effective_scale;
                    }

                    out.push(bounds);
                }

                if out.len() >= 2 {
                    let total_row_width =
                        out[0].width() + out[1].width() + Self::GAP * effective_scale * 2.0;
                    for bound in &mut out {
                        bound.translate(Vector::new(-total_row_width / 4.0, 0.0));
                    }
                }
            }
            PageLayout::DoublePageTitlePage => {
                let mut pos: Vector<f32> = Vector::zero();
                let Some(Ok(first_page)) = pages.next() else {
                    return Ok(out);
                };
                let mut bounds: Rect<f32> = first_page.bounds()?.into();
                bounds.translate((vsize - bounds.size()).scaled(0.5));
                bounds.translate(translation.scaled(effective_scale));
                bounds = bounds.scaled(effective_scale);
                out.push(bounds);
                pos.y += Self::GAP * effective_scale + bounds.size().y;

                for (i, page) in pages.flatten().enumerate() {
                    let mut bounds: Rect<f32> = page.bounds()?.into();
                    bounds.translate(pos);
                    bounds.translate((vsize - bounds.size()).scaled(0.5));
                    bounds.translate(translation.scaled(effective_scale));
                    bounds = bounds.scaled(effective_scale);

                    if i.is_odd() {
                        pos.y += bounds.size().y;
                        pos.y += Self::GAP * effective_scale;
                        pos.x = 0.0;
                    } else {
                        pos.x += bounds.size().x;
                        pos.x += Self::GAP * effective_scale;
                    }

                    out.push(bounds);
                }

                if out.len() >= 3 {
                    let pages_below_width =
                        out[1].width() + out[2].width() + Self::GAP * effective_scale * 2.0;
                    out[0].translate(Vector::new(pages_below_width / 4.0, 0.0));

                    for bound in &mut out {
                        bound.translate(Vector::new(-pages_below_width / 4.0, 0.0));
                    }
                } else if out.len() == 2 {
                    let half_width = first_page.bounds()?.width() / 2.0;
                    out[1].translate(Vector::new(half_width, 0.0));
                }
            }
            PageLayout::Presentation => {
                let mut pos: Vector<f32> = Vector::zero();
                let mut prev_bounds = Rect::default();
                for (i, page) in pages.flatten().enumerate() {
                    let mut bounds: Rect<f32> = page.bounds()?.into();
                    bounds.translate((vsize - bounds.size()).scaled(0.5));
                    bounds.translate(translation.scaled(effective_scale));
                    bounds = bounds.scaled(effective_scale);
                    if i != 0 {
                        pos.y += (prev_bounds.height() + bounds.height()) / 2.0;
                    }
                    bounds.translate(pos);

                    pos.y += Self::GAP * effective_scale;
                    prev_bounds = bounds;

                    out.push(bounds);
                }

                if !out.is_empty() {
                    let viewport_center = vsize.scaled(0.5);
                    let closest = out
                        .iter()
                        .enumerate()
                        .min_by(|(_, a), (_, b)| {
                            (a.center() - viewport_center)
                                .norm_squared()
                                .partial_cmp(&(b.center() - viewport_center).norm_squared())
                                .unwrap_or(std::cmp::Ordering::Equal)
                        })
                        .map(|(i, _)| i)
                        .unwrap();

                    let snap_offset = viewport_center - out[closest].center();
                    for rect in &mut out {
                        rect.translate(snap_offset);
                    }
                }
            }
        }
        Ok(out)
    }

    /// Returns the inclusive range of page indices that are laid out side by side in the same
    /// row (a "spread") as `page_idx`. For single-page layouts this is just `page_idx` itself.
    pub fn spread_page_range(
        &self,
        page_idx: usize,
        page_count: usize,
    ) -> std::ops::RangeInclusive<usize> {
        if page_count == 0 {
            return 0..=0;
        }
        let last = page_count - 1;
        let page_idx = page_idx.min(last);
        // Page 0 of the title-page layout sits on its own row.
        let start = match self {
            PageLayout::SinglePage | PageLayout::Presentation => page_idx,
            PageLayout::DoublePage => page_idx - (page_idx % 2),
            PageLayout::DoublePageTitlePage if page_idx == 0 => 0,
            PageLayout::DoublePageTitlePage => page_idx - ((page_idx - 1) % 2),
        };
        let end = match self {
            PageLayout::SinglePage | PageLayout::Presentation => start,
            PageLayout::DoublePageTitlePage if page_idx == 0 => 0,
            _ => (start + 1).min(last),
        };
        start..=end
    }

    /// Returns the bounding box of the spread containing `page_idx` as laid out at `scale`.
    fn spread_rect(
        &self,
        doc: &Document,
        page_idx: usize,
        scale: f32,
        fractional_scale: f32,
        viewport: Size<f32>,
    ) -> Result<Rect<f32>> {
        let rects = self.pages_rects(
            doc.pages()?,
            Vector::zero(),
            scale,
            fractional_scale,
            viewport,
        )?;
        if rects.is_empty() {
            return Err(anyhow!("There are no pages"));
        }
        let range = self.spread_page_range(page_idx, rects.len());
        Ok(rects[range]
            .iter()
            .fold(rects[page_idx.min(rects.len() - 1)], |acc, rect| {
                acc.union(rect)
            }))
    }

    /// Returns the scale and translation that fit and center the spread containing `page_idx` in
    /// the viewport. Unlike fitting a single page, this keeps both pages of a two-page spread
    /// fully visible and centered.
    pub fn zoom_fit(
        &self,
        doc: &Document,
        page_idx: usize,
        fractional_scale: f32,
        viewport: Size<f32>,
    ) -> Result<(f32, Vector<f32>)> {
        if viewport.width <= 0.0 || viewport.height <= 0.0 {
            return Err(anyhow!("Cannot fit pages in a zero-sized viewport"));
        }
        // The spread's size is linear in the effective scale, so measure it at scale 1.
        let reference = self.spread_rect(doc, page_idx, 1.0, 1.0, viewport)?;
        let size = reference.size();
        if size.x <= 0.0 || size.y <= 0.0 {
            return Err(anyhow!("Cannot fit a spread with zero size"));
        }
        let effective_scale = (viewport.width / size.x).min(viewport.height / size.y);
        let scale = effective_scale / fractional_scale;

        // Recompute at the target scale since scaling around each page's center can shift the
        // bounding box when the pages have different sizes.
        let spread = self.spread_rect(doc, page_idx, scale, fractional_scale, viewport)?;
        let viewport_center = Vector::new(viewport.width, viewport.height).scaled(0.5);
        // `pages_rects` is called with `-translation` when rendering, so a spread below the
        // viewport center needs a positive translation (same convention as
        // `translation_for_page`).
        let translation = (spread.center() - viewport_center).scaled(1.0 / effective_scale);
        Ok((scale, translation))
    }

    /// Returns the translation that would leave the page at [page_idx] visible on the screen. If
    /// `page_idx > doc.page_count()` this will move to the last page.
    pub fn translation_for_page(
        &self,
        doc: &Document,
        scale: f32,
        fractional_scale: f32,
        page_idx: usize,
        viewport: Size<f32>,
    ) -> Result<Vector<f32>> {
        let rects = self.pages_rects(
            doc.pages()?,
            Vector::zero(),
            scale,
            fractional_scale,
            viewport,
        )?;
        let rect = rects
            .get(page_idx)
            .ok_or(anyhow!("Page index {page_idx} out of bounds"))?;
        let viewport_center = Vector::new(viewport.width, viewport.height).scaled(0.5);
        Ok((rect.center() - viewport_center).scaled(1.0 / (scale * fractional_scale)))
    }

    pub fn current_page_index(
        &self,
        doc: &Document,
        translation: Vector<f32>,
        viewport: Size<f32>,
    ) -> Result<usize> {
        let rects = self.pages_rects(doc.pages()?, -translation, 1.0, 1.0, viewport)?;
        let mut closest = 0;
        let viewport: Vector<_> = viewport.into();
        if rects.is_empty() {
            return Err(anyhow!("There are no pages"));
        }
        for (i, rect) in rects.iter().enumerate() {
            if (rect.center() - viewport.scaled(0.5)).norm_squared()
                < (rects[closest].center() - viewport.scaled(0.5)).norm_squared()
            {
                closest = i;
            }
        }
        Ok(closest)
    }

    pub fn center_of_page(
        &self,
        doc: &Document,
        translation: Vector<f32>,
        viewport: Size<f32>,
    ) -> Result<Rect<f32>> {
        let rects = self.pages_rects(doc.pages()?, translation, 1.0, 1.0, viewport)?;
        let idx = self.current_page_index(doc, translation, viewport)?;
        Ok(rects[idx])
    }

    pub fn center_of_page_above(
        &self,
        doc: &Document,
        translation: Vector<f32>,
        viewport: Size<f32>,
    ) -> Result<Rect<f32>> {
        let rects = self.pages_rects(doc.pages()?, translation, 1.0, 1.0, viewport)?;
        let mut idx = self.current_page_index(doc, translation, viewport)?;
        idx = (match self {
            PageLayout::SinglePage => idx.saturating_sub(1),
            PageLayout::DoublePage => idx.saturating_sub(2),
            PageLayout::DoublePageTitlePage => idx.saturating_sub(2),
            PageLayout::Presentation => idx.saturating_sub(1),
        })
        .clamp(0, rects.len() - 1);
        Ok(rects[idx])
    }

    pub fn center_of_page_below(
        &self,
        doc: &Document,
        translation: Vector<f32>,
        viewport: Size<f32>,
    ) -> Result<Rect<f32>> {
        let rects = self.pages_rects(doc.pages()?, translation, 1.0, 1.0, viewport)?;
        let mut idx = self.current_page_index(doc, translation, viewport)?;
        idx = (match self {
            PageLayout::SinglePage => idx + 1,
            PageLayout::DoublePage => idx + 2,
            PageLayout::DoublePageTitlePage => {
                if idx == 0 {
                    idx + 1
                } else {
                    idx + 2
                }
            }
            PageLayout::Presentation => idx + 1,
        })
        .clamp(0, rects.len() - 1);
        Ok(rects[idx])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mupdf::Document;

    #[test]
    fn test_translation_for_page() -> Result<()> {
        let doc = Document::open("assets/links.pdf")?;
        let layout = PageLayout::SinglePage;
        let viewport = Size::new(800.0, 600.0);
        let scale = 1.0;
        let fractional_scale = 1.0;

        let t0 = layout.translation_for_page(&doc, scale, fractional_scale, 0, viewport)?;
        let t1 = layout.translation_for_page(&doc, scale, fractional_scale, 1, viewport)?;
        let t2 = layout.translation_for_page(&doc, scale, fractional_scale, 2, viewport)?;

        // Page 0 should need minimal/no translation to be centered
        // (it's already centered when translation=0)
        assert!(
            t0.norm_squared() < 1.0,
            "Page 0 should be near viewport center with zero translation, got {:?}",
            t0
        );

        // Page 1 is below the viewport center, so we need positive y translation
        // (self.translation is negated before being passed to pages_rects in view())
        assert!(
            t1.y > t0.y,
            "Page 1 should require positive y translation, got {:?}",
            t1
        );

        // Page 2 is even further down
        assert!(
            t2.y > t1.y,
            "Page 2 should require more positive y translation than page 1, got {:?}",
            t2
        );

        // Verify that applying the NEGATED translation to pages_rects centers the page
        // (view() passes -self.translation to pages_rects)
        let rects = layout.pages_rects(doc.pages()?, -t1, scale, fractional_scale, viewport)?;
        let viewport_center = Vector::new(viewport.width, viewport.height).scaled(0.5);
        let page1_center = rects[1].center();
        let diff = (page1_center - viewport_center).norm_squared();
        assert!(
            diff < 1.0,
            "Page 1 should be centered after applying -translation, got center {:?}, viewport center {:?}",
            page1_center,
            viewport_center
        );

        Ok(())
    }
}
