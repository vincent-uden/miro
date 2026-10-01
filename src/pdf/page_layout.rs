use anyhow::{anyhow, Result};
use iced::Size;
use serde::{Deserialize, Serialize};
use strum::EnumString;

use crate::geometry::{Rect, Vector};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, EnumString, Default, PartialEq, Eq)]
pub enum PageLayoutKind {
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq, Hash)]
pub enum PageRotation {
    #[default]
    Deg0,
    Deg90,
    Deg180,
    Deg270,
}

impl PageRotation {
    pub fn clockwise(self) -> Self {
        match self {
            Self::Deg0 => Self::Deg90,
            Self::Deg90 => Self::Deg180,
            Self::Deg180 => Self::Deg270,
            Self::Deg270 => Self::Deg0,
        }
    }

    pub fn counter_clockwise(self) -> Self {
        match self {
            Self::Deg0 => Self::Deg270,
            Self::Deg270 => Self::Deg180,
            Self::Deg180 => Self::Deg90,
            Self::Deg90 => Self::Deg0,
        }
    }

    pub fn rotated_size(self, size: Vector<f32>) -> Vector<f32> {
        match self {
            Self::Deg90 | Self::Deg270 => Vector::new(size.y, size.x),
            Self::Deg0 | Self::Deg180 => size,
        }
    }

    /// Map PDF coordinates to the page's normalized, rotated coordinate space.
    pub fn to_rotated(self, point: Vector<f32>, bounds: Rect<f32>) -> Vector<f32> {
        match self {
            Self::Deg0 => Vector::new(point.x - bounds.x0.x, point.y - bounds.x0.y),
            Self::Deg90 => Vector::new(bounds.x1.y - point.y, point.x - bounds.x0.x),
            Self::Deg180 => Vector::new(bounds.x1.x - point.x, bounds.x1.y - point.y),
            Self::Deg270 => Vector::new(point.y - bounds.x0.y, bounds.x1.x - point.x),
        }
    }

    /// Map normalized, rotated page coordinates back to PDF coordinates.
    pub fn from_rotated(self, point: Vector<f32>, bounds: Rect<f32>) -> Vector<f32> {
        match self {
            Self::Deg0 => Vector::new(point.x + bounds.x0.x, point.y + bounds.x0.y),
            Self::Deg90 => Vector::new(point.y + bounds.x0.x, bounds.x1.y - point.x),
            Self::Deg180 => Vector::new(bounds.x1.x - point.x, bounds.x1.y - point.y),
            Self::Deg270 => Vector::new(bounds.x1.x - point.y, point.x + bounds.x0.y),
        }
    }
}

/// The page arrangement and per-page reading rotations for a PDF.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PageLayout {
    #[serde(default)]
    pub layout: PageLayoutKind,
    #[serde(default)]
    rotations: Vec<PageRotation>,
}

impl PageLayout {
    const GAP: f32 = 10.0;

    pub fn new(layout: PageLayoutKind) -> Self {
        Self {
            layout,
            rotations: Vec::new(),
        }
    }

    pub fn rotation(&self, page_idx: usize) -> PageRotation {
        self.rotations.get(page_idx).copied().unwrap_or_default()
    }

    pub fn rotate_page_clockwise(&mut self, page_idx: usize) {
        self.set_page_rotation(page_idx, self.rotation(page_idx).clockwise());
    }

    pub fn rotate_page_counter_clockwise(&mut self, page_idx: usize) {
        self.set_page_rotation(page_idx, self.rotation(page_idx).counter_clockwise());
    }

    pub fn rotate_all_pages_clockwise(&mut self, page_count: usize) {
        self.rotations.resize(page_count, PageRotation::Deg0);
        for rotation in &mut self.rotations {
            *rotation = rotation.clockwise();
        }
    }

    pub fn rotate_all_pages_counter_clockwise(&mut self, page_count: usize) {
        self.rotations.resize(page_count, PageRotation::Deg0);
        for rotation in &mut self.rotations {
            *rotation = rotation.counter_clockwise();
        }
    }

    fn set_page_rotation(&mut self, page_idx: usize, rotation: PageRotation) {
        self.rotations.resize(page_idx + 1, PageRotation::Deg0);
        self.rotations[page_idx] = rotation;
    }

    fn page_rect(&self, page_idx: usize, bounds: Rect<f32>) -> Rect<f32> {
        let size = self.rotation(page_idx).rotated_size(bounds.size());
        Rect {
            x0: Vector::zero(),
            x1: size,
        }
    }

    fn append_spread_rows(
        out: &mut Vec<Rect<f32>>,
        page_sizes: &[Vector<f32>],
        center: Vector<f32>,
        effective_scale: f32,
        previous_row_height: Option<f32>,
    ) {
        let mut row_center_y = center.y;
        let mut previous_row_height = previous_row_height;
        for row in page_sizes.chunks(2) {
            let row_height = row.iter().map(|size| size.y).fold(0.0, f32::max);
            if let Some(previous_height) = previous_row_height {
                row_center_y +=
                    (previous_height / 2.0 + Self::GAP + row_height / 2.0) * effective_scale;
            }

            let row_width = row.iter().map(|size| size.x).sum::<f32>()
                + Self::GAP * row.len().saturating_sub(1) as f32;
            let mut page_x = center.x - row_width * effective_scale / 2.0;
            for size in row {
                let screen_size = size.scaled(effective_scale);
                let page_y = row_center_y - screen_size.y / 2.0;
                out.push(Rect::from_pos_size(
                    Vector::new(page_x, page_y),
                    screen_size,
                ));
                page_x += screen_size.x + Self::GAP * effective_scale;
            }
            previous_row_height = Some(row_height);
        }
    }

    /// Returns visible pages and their bounding boxes relative to the widgets origin. A translation
    /// of (0,0) should result in the first page row being centered on the screen. Scale is applied
    /// after translation with respect to the center of the screen. Thus zooming doesn't move the
    /// doucment.
    pub fn pages_rects(
        &self,
        page_bounds: &[Rect<f32>],
        translation: Vector<f32>, // In document space
        scale: f32,
        fractional_scale: f32,
        viewport: Size<f32>,
    ) -> Result<Vec<Rect<f32>>> {
        let _span = tracy_client::span!("Pages rects");
        let page_sizes: Vec<_> = page_bounds
            .iter()
            .enumerate()
            .map(|(i, bounds)| self.page_rect(i, *bounds).size())
            .collect();
        let mut out: Vec<Rect<f32>> = vec![];
        let vsize: Vector<_> = viewport.into();
        let effective_scale = scale * fractional_scale;
        match self.layout {
            PageLayoutKind::SinglePage => {
                let mut pos: Vector<f32> = Vector::zero();
                let mut prev_bounds = Rect::default();
                for (i, size) in page_sizes.iter().copied().enumerate() {
                    let mut bounds = Rect::from_pos_size(Vector::zero(), size);
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
            PageLayoutKind::DoublePage => {
                let center =
                    Vector::new(vsize.x * 0.5, vsize.y * 0.5) + translation.scaled(effective_scale);
                Self::append_spread_rows(&mut out, &page_sizes, center, effective_scale, None);
            }
            PageLayoutKind::DoublePageTitlePage => {
                let Some(first_size) = page_sizes.first().copied() else {
                    return Ok(out);
                };
                let center =
                    Vector::new(vsize.x * 0.5, vsize.y * 0.5) + translation.scaled(effective_scale);
                let first_size = first_size.scaled(effective_scale);
                out.push(Rect::from_pos_size(
                    center - first_size.scaled(0.5),
                    first_size,
                ));
                Self::append_spread_rows(
                    &mut out,
                    &page_sizes[1..],
                    center,
                    effective_scale,
                    Some(page_sizes[0].y),
                );
            }
            PageLayoutKind::Presentation => {
                let mut pos: Vector<f32> = Vector::zero();
                let mut prev_bounds = Rect::default();
                for (i, size) in page_sizes.iter().copied().enumerate() {
                    let mut bounds = Rect::from_pos_size(Vector::zero(), size);
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
        let start = match self.layout {
            PageLayoutKind::SinglePage | PageLayoutKind::Presentation => page_idx,
            PageLayoutKind::DoublePage => page_idx - (page_idx % 2),
            PageLayoutKind::DoublePageTitlePage if page_idx == 0 => 0,
            PageLayoutKind::DoublePageTitlePage => page_idx - ((page_idx - 1) % 2),
        };
        let end = match self.layout {
            PageLayoutKind::SinglePage | PageLayoutKind::Presentation => start,
            PageLayoutKind::DoublePageTitlePage if page_idx == 0 => 0,
            _ => (start + 1).min(last),
        };
        start..=end
    }

    /// Returns the bounding box of the spread containing `page_idx` as laid out at `scale`.
    fn spread_rect(
        &self,
        page_bounds: &[Rect<f32>],
        page_idx: usize,
        scale: f32,
        fractional_scale: f32,
        viewport: Size<f32>,
    ) -> Result<Rect<f32>> {
        let _span = tracy_client::span!("Spread rect");
        let rects = self.pages_rects(
            page_bounds,
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
        page_bounds: &[Rect<f32>],
        page_idx: usize,
        fractional_scale: f32,
        viewport: Size<f32>,
    ) -> Result<(f32, Vector<f32>)> {
        let _span = tracy_client::span!("Zoom fit");
        if viewport.width <= 0.0 || viewport.height <= 0.0 {
            return Err(anyhow!("Cannot fit pages in a zero-sized viewport"));
        }
        // The spread's size is linear in the effective scale, so measure it at scale 1.
        let reference = self.spread_rect(page_bounds, page_idx, 1.0, 1.0, viewport)?;
        let size = reference.size();
        if size.x <= 0.0 || size.y <= 0.0 {
            return Err(anyhow!("Cannot fit a spread with zero size"));
        }
        let effective_scale = (viewport.width / size.x).min(viewport.height / size.y);
        let scale = effective_scale / fractional_scale;

        // Recompute at the target scale since scaling around each page's center can shift the
        // bounding box when the pages have different sizes.
        let spread = self.spread_rect(page_bounds, page_idx, scale, fractional_scale, viewport)?;
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
        page_bounds: &[Rect<f32>],
        scale: f32,
        fractional_scale: f32,
        page_idx: usize,
        viewport: Size<f32>,
    ) -> Result<Vector<f32>> {
        let _span = tracy_client::span!("Translation for page");
        let rects = self.pages_rects(
            page_bounds,
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
        page_bounds: &[Rect<f32>],
        translation: Vector<f32>,
        viewport: Size<f32>,
    ) -> Result<usize> {
        let _span = tracy_client::span!("Current page index");
        let rects = self.pages_rects(page_bounds, -translation, 1.0, 1.0, viewport)?;
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
        page_bounds: &[Rect<f32>],
        translation: Vector<f32>,
        viewport: Size<f32>,
    ) -> Result<Rect<f32>> {
        let _span = tracy_client::span!("Center of page");
        let rects = self.pages_rects(page_bounds, translation, 1.0, 1.0, viewport)?;
        let idx = self.current_page_index(page_bounds, translation, viewport)?;
        Ok(rects[idx])
    }

    pub fn center_of_page_above(
        &self,
        page_bounds: &[Rect<f32>],
        translation: Vector<f32>,
        viewport: Size<f32>,
    ) -> Result<Rect<f32>> {
        let _span = tracy_client::span!("Center of page above");
        let rects = self.pages_rects(page_bounds, translation, 1.0, 1.0, viewport)?;
        let mut idx = self.current_page_index(page_bounds, translation, viewport)?;
        idx = (match self.layout {
            PageLayoutKind::SinglePage => idx.saturating_sub(1),
            PageLayoutKind::DoublePage => idx.saturating_sub(2),
            PageLayoutKind::DoublePageTitlePage => idx.saturating_sub(2),
            PageLayoutKind::Presentation => idx.saturating_sub(1),
        })
        .clamp(0, rects.len() - 1);
        Ok(rects[idx])
    }

    pub fn center_of_page_below(
        &self,
        page_bounds: &[Rect<f32>],
        translation: Vector<f32>,
        viewport: Size<f32>,
    ) -> Result<Rect<f32>> {
        let _span = tracy_client::span!("Center of page below");
        let rects = self.pages_rects(page_bounds, translation, 1.0, 1.0, viewport)?;
        let mut idx = self.current_page_index(page_bounds, translation, viewport)?;
        idx = (match self.layout {
            PageLayoutKind::SinglePage => idx + 1,
            PageLayoutKind::DoublePage => idx + 2,
            PageLayoutKind::DoublePageTitlePage => {
                if idx == 0 {
                    idx + 1
                } else {
                    idx + 2
                }
            }
            PageLayoutKind::Presentation => idx + 1,
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
    fn spread_rows_center_differently_sized_pages_without_overlap() {
        let page_sizes = [Vector::new(100.0, 200.0), Vector::new(400.0, 150.0)];
        let center = Vector::new(500.0, 300.0);
        let mut rects = Vec::new();

        PageLayout::append_spread_rows(&mut rects, &page_sizes, center, 1.0, None);

        assert_eq!(rects.len(), 2);
        assert!(rects[0].x1.x < rects[1].x0.x);
        let spread = rects[0].union(&rects[1]);
        assert!((spread.center().x - center.x).abs() < 1e-5);
        assert!((rects[0].center().y - rects[1].center().y).abs() < 1e-5);
    }

    #[test]
    fn test_translation_for_page() -> Result<()> {
        let doc = Document::open("assets/links.pdf")?;
        let page_bounds: Vec<Rect<f32>> = doc
            .pages()?
            .flatten()
            .map(|page| Ok(Rect::from(page.bounds()?)))
            .collect::<Result<_>>()?;
        let layout = PageLayout::new(PageLayoutKind::SinglePage);
        let viewport = Size::new(800.0, 600.0);
        let scale = 1.0;
        let fractional_scale = 1.0;

        let t0 = layout.translation_for_page(&page_bounds, scale, fractional_scale, 0, viewport)?;
        let t1 = layout.translation_for_page(&page_bounds, scale, fractional_scale, 1, viewport)?;
        let t2 = layout.translation_for_page(&page_bounds, scale, fractional_scale, 2, viewport)?;

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
        let rects = layout.pages_rects(&page_bounds, -t1, scale, fractional_scale, viewport)?;
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
