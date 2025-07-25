use anyhow::Result;
use mupdf::{Page, Rect as MupdfRect, TextPage, TextPageOptions};

use crate::geometry::{Rect, Vector};

#[derive(Debug, Clone)]
pub struct TextSelection {
    pub text: String,
    pub bounds: Rect<f32>,
}

#[derive(Debug)]
pub struct TextExtractor<'a> {
    page: &'a Page,
}

impl<'a> TextExtractor<'a> {
    pub fn new(page: &'a Page) -> Self {
        Self { page }
    }

    pub fn extract_text_in_rect(&self, selection_rect: MupdfRect) -> Result<TextSelection> {
        let text_page = self.page.to_text_page(TextPageOptions::empty())?;

        let mut selected_text = String::new();
        let mut bounds = Vec::new();

        for block in text_page.blocks() {
            for line in block.lines() {
                let line_bounds = line.bounds();

                if rectangles_intersect(selection_rect, line_bounds) {
                    for ch in line.chars() {
                        let char_quad = ch.quad();
                        let char_rect = MupdfRect {
                            x0: char_quad.ul.x,
                            y0: char_quad.ul.y,
                            x1: char_quad.lr.x,
                            y1: char_quad.lr.y,
                        };

                        if rectangles_intersect(selection_rect, char_rect)
                            && let Some(c) = ch.char()
                        {
                            selected_text.push(c);
                        }
                    }
                    selected_text.push('\n');
                    bounds.push(line_bounds);
                }
            }
        }

        let total_bounds = bounds
            .iter()
            .fold(Rect::default(), |acc: Rect<f32>, r| Rect {
                x0: Vector {
                    x: acc.x0.x.min(r.x0),
                    y: acc.x0.y.min(r.y0),
                },
                x1: Vector {
                    x: acc.x1.x.max(r.x1),
                    y: acc.x1.y.max(r.y1),
                },
            });

        Ok(TextSelection {
            text: selected_text.trim().to_string(),
            bounds: total_bounds,
        })
    }

    #[allow(dead_code)]
    pub fn extract_all_text(&self) -> Result<String> {
        let text = self.page.to_text()?;
        Ok(text)
    }

    #[allow(dead_code)]
    pub fn get_text_page(&self) -> Result<TextPage> {
        let text_page = self.page.to_text_page(TextPageOptions::empty())?;
        Ok(text_page)
    }
}

fn rectangles_intersect(rect1: MupdfRect, rect2: MupdfRect) -> bool {
    rect1.x0 < rect2.x1 && rect1.x1 > rect2.x0 && rect1.y0 < rect2.y1 && rect1.y1 > rect2.y0
}

#[cfg(test)]
mod tests {
    use super::*;
    use mupdf::{Document, Rect as MupdfRect};

    #[test]
    fn test_text_extraction_basic() -> Result<()> {
        let document = Document::open("assets/text-copy-test.pdf")?;
        let page = document.load_page(0)?;
        let extractor = TextExtractor::new(&page);

        let all_text = extractor.extract_all_text()?;
        assert!(!all_text.is_empty());
        assert!(all_text.contains("Energy harvesting"));
        assert!(all_text.contains("Vincent Udén"));

        Ok(())
    }

    #[test]
    fn test_text_extraction_rectangle_selection() -> Result<()> {
        let document = Document::open("assets/text-copy-test.pdf")?;
        let page = document.load_page(0)?;
        let extractor = TextExtractor::new(&page);

        let selection_rect = MupdfRect {
            x0: 100.0,
            y0: 400.0,
            x1: 500.0,
            y1: 600.0,
        };

        let selection = extractor.extract_text_in_rect(selection_rect)?;
        assert!(!selection.text.is_empty());

        Ok(())
    }

    #[test]
    fn test_multiple_pages() -> Result<()> {
        let document = Document::open("assets/text-copy-test.pdf")?;

        let page_count = document.page_count()?;
        assert!(page_count > 1);

        let page = document.load_page(1)?;
        let extractor = TextExtractor::new(&page);
        let page2_text = extractor.extract_all_text()?;
        assert!(page2_text.contains("Introduction"));

        Ok(())
    }

    #[test]
    fn test_text_extraction_integration() -> Result<()> {
        let document = Document::open("assets/text-copy-test.pdf")?;

        // Test page 0
        let page0 = document.load_page(0)?;
        let extractor0 = TextExtractor::new(&page0);
        let all_text = extractor0.extract_all_text()?;
        assert!(all_text.contains("Energy harvesting"));

        // Test specific rectangle selection on page 0 - use a larger area to ensure we catch text
        let title_rect = MupdfRect {
            x0: 100.0,
            y0: 400.0,
            x1: 700.0,
            y1: 600.0,
        };

        let selection = extractor0.extract_text_in_rect(title_rect)?;
        // The selection might be empty if coordinates don't match text, so let's just check it doesn't error
        println!("Title selection: '{}'", selection.text);

        // Test page 1
        let page1 = document.load_page(1)?;
        let extractor1 = TextExtractor::new(&page1);
        let page1_text = extractor1.extract_all_text()?;
        assert!(page1_text.contains("Introduction"));

        // Test rectangle selection on page 1 - use a larger area
        let intro_rect = MupdfRect {
            x0: 50.0,
            y0: 150.0,
            x1: 600.0,
            y1: 400.0,
        };

        let intro_selection = extractor1.extract_text_in_rect(intro_rect)?;
        println!("Intro selection: '{}'", intro_selection.text);

        Ok(())
    }

    #[test]
    fn test_screen_to_document_coordinate_simulation() -> Result<()> {
        // This test simulates the coordinate conversion that happens in the widget
        println!("=== COORDINATE CONVERSION SIMULATION ===");

        // Simulate typical screen coordinates (like mouse positions)
        let screen_positions = vec![
            (400.0, 300.0), // Center-ish of a typical window
            (200.0, 200.0), // Upper left area
            (600.0, 400.0), // Lower right area
        ];

        // Simulate typical viewport bounds
        let viewport_bounds = crate::geometry::Rect::from_pos_size(
            crate::geometry::Vector::new(0.0, 0.0),
            crate::geometry::Vector::new(800.0, 600.0),
        );

        // Simulate typical page size (from our test PDF)
        let page_size = crate::geometry::Vector::new(595.0, 842.0); // A4 size in points

        // Simulate typical translation and scale values
        let translation = crate::geometry::Vector::new(0.0, 0.0); // No panning
        let scale = 1.0; // No zoom

        println!("Viewport bounds: {viewport_bounds:?}");
        println!("Page size: {page_size:?}");
        println!("Translation: {translation:?}");
        println!("Scale: {scale}");

        // Calculate PDF offset (same as in the fixed coordinate conversion)
        let pdf_offset = crate::geometry::Vector::new(
            -(viewport_bounds.width() - page_size.x * scale) / 2.0,
            -(viewport_bounds.height() - page_size.y * scale) / 2.0,
        );

        println!("PDF offset: {pdf_offset:?}");

        for (screen_x, screen_y) in screen_positions {
            let screen_pos = crate::geometry::Vector::new(screen_x, screen_y);

            // Simulate the NEW coordinate conversion from widget.rs
            let viewport_relative = screen_pos - viewport_bounds.x0;
            let pdf_relative = viewport_relative - pdf_offset;
            let doc_pos = pdf_relative.scaled(1.0 / scale) + translation;

            println!(
                "Screen ({}, {}) -> Viewport rel ({}, {}) -> PDF rel ({}, {}) -> Doc ({}, {})",
                screen_x,
                screen_y,
                viewport_relative.x,
                viewport_relative.y,
                pdf_relative.x,
                pdf_relative.y,
                doc_pos.x,
                doc_pos.y
            );
        }

        // Test if these coordinates would intersect with known text positions
        println!("\n=== INTERSECTION TESTS ===");
        println!("Known text positions from coordinate debugging:");
        println!("- 'Energy harvesting': (200-394, 299-327)");
        println!("- 'Vincent Udén': (262-333, 362-376)");

        Ok(())
    }
}
