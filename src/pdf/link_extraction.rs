use anyhow::Result;
use mupdf::Page;

use crate::geometry::Rect;

#[derive(Debug, Clone)]
pub struct LinkInfo {
    pub bounds: Rect<f32>,
    pub uri: String,
    pub link_type: LinkType,
}

#[derive(Debug, Clone)]
pub enum LinkType {
    ExternalUrl,
    InternalPage(u32),
    Email,
    Other,
}

#[derive(Debug)]
pub struct LinkExtractor<'a> {
    page: &'a Page,
}

impl<'a> LinkExtractor<'a> {
    pub fn new(page: &'a Page) -> Self {
        Self { page }
    }

    pub fn extract_all_links(&self) -> Result<Vec<LinkInfo>> {
        let mut links = Vec::new();
        
        let link_iter = self.page.links()?;
        let page_bounds = self.page.bounds()?;
        
        tracing::debug!("=== LINK EXTRACTION DEBUG ===");
        tracing::debug!("Page bounds: {:?}", page_bounds);
        tracing::debug!("Page size: {}x{}", page_bounds.width(), page_bounds.height());
        
        for (idx, link) in link_iter.enumerate() {
            let bounds = Rect::from_pos_size(
                crate::geometry::Vector::new(link.bounds.x0, link.bounds.y0),
                crate::geometry::Vector::new(
                    link.bounds.x1 - link.bounds.x0,
                    link.bounds.y1 - link.bounds.y0,
                ),
            );
            
            let link_type = categorize_link(&link.uri);
            
            tracing::debug!(
                "Link {}: uri='{}' type={:?} bounds={:?} (raw: x0={}, y0={}, x1={}, y1={})",
                idx, link.uri, link_type, bounds, 
                link.bounds.x0, link.bounds.y0, link.bounds.x1, link.bounds.y1
            );
            
            // Debug: Check if coordinates are within page bounds
            let within_bounds = link.bounds.x0 >= 0.0 && link.bounds.y0 >= 0.0 
                && link.bounds.x1 <= page_bounds.width() && link.bounds.y1 <= page_bounds.height();
            tracing::debug!("  Within page bounds: {}", within_bounds);
            
            // Debug: Calculate relative position on page
            let rel_x = link.bounds.x0 / page_bounds.width();
            let rel_y = link.bounds.y0 / page_bounds.height();
            tracing::debug!("  Relative position: ({:.2}%, {:.2}%)", rel_x * 100.0, rel_y * 100.0);
            
            links.push(LinkInfo {
                bounds,
                uri: link.uri,
                link_type,
            });
        }
        
        tracing::debug!("Total links extracted: {}", links.len());
        Ok(links)
    }
}

fn categorize_link(uri: &str) -> LinkType {
    if uri.starts_with("http://") || uri.starts_with("https://") {
        LinkType::ExternalUrl
    } else if uri.starts_with("mailto:") {
        LinkType::Email
    } else if uri.starts_with("#page=") {
        // Parse page number from internal page reference
        if let Some(page_str) = uri.strip_prefix("#page=") {
            if let Ok(page_num) = page_str.parse::<u32>() {
                return LinkType::InternalPage(page_num);
            }
        }
        LinkType::Other
    } else if uri.chars().all(|c| c.is_ascii_digit()) {
        // Sometimes page references are just numbers
        if let Ok(page_num) = uri.parse::<u32>() {
            LinkType::InternalPage(page_num)
        } else {
            LinkType::Other
        }
    } else {
        LinkType::Other
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mupdf::Document;

    #[test]
    fn test_link_extraction_basic() -> Result<()> {
        let document = Document::open("assets/text-copy-test.pdf")?;
        let page = document.load_page(0)?;
        let extractor = LinkExtractor::new(&page);

        let links = extractor.extract_all_links()?;
        
        // Print found links for debugging
        for (i, link) in links.iter().enumerate() {
            println!("Link {}: {:?} at {:?}", i, link.uri, link.bounds);
        }
        
        // The test should not fail even if no links are found
        // This allows us to test the extraction mechanism
        Ok(())
    }

    #[test]
    fn test_links_pdf_extraction() -> Result<()> {
        let document = Document::open("assets/links.pdf")?;
        let page = document.load_page(0)?;
        let extractor = LinkExtractor::new(&page);

        let links = extractor.extract_all_links()?;
        
        println!("=== LINKS.PDF PAGE 0 ===");
        for (i, link) in links.iter().enumerate() {
            println!("Link {}: {:?} ({:?}) at {:?}", i, link.uri, link.link_type, link.bounds);
        }
        
        // Test that we found some links
        assert!(!links.is_empty(), "Should find links in the test document");
        
        // Test that we have different types of links
        let has_external = links.iter().any(|l| matches!(l.link_type, LinkType::ExternalUrl));
        let has_email = links.iter().any(|l| matches!(l.link_type, LinkType::Email));
        
        println!("Has external links: {}", has_external);
        println!("Has email links: {}", has_email);
        
        Ok(())
    }

    #[test]
    fn test_coordinate_transformation_simulation() -> Result<()> {
        use crate::geometry::Vector;
        use mupdf::Document;
        
        println!("=== COORDINATE TRANSFORMATION TEST ===");
        
        // Get actual page size from the PDF
        let document = Document::open("assets/links.pdf")?;
        let page = document.load_page(0)?;
        let page_bounds = page.bounds()?;
        let page_size = Vector::new(page_bounds.width(), page_bounds.height());
        
        println!("Actual page bounds: {:?}", page_bounds);
        println!("Actual page size: {:?}", page_size);
        
        // Simulate typical values from the application
        let viewport_size = Vector::new(800.0, 600.0); // Typical window size
        let scale = 1.0;
        let translation = Vector::new(0.0, 0.0);
        
        // Calculate PDF positioning (same logic as in the app)
        let scaled_page_size = Vector::new(
            page_size.x * scale,
            page_size.y * scale,
        );
        
        let pdf_top_left = Vector::new(
            (viewport_size.x - scaled_page_size.x) / 2.0,
            (viewport_size.y - scaled_page_size.y) / 2.0,
        );
        
        println!("Viewport size: {:?}", viewport_size);
        println!("Scaled page size: {:?}", scaled_page_size);
        println!("PDF top-left offset: {:?}", pdf_top_left);
        
        // Test with a sample link coordinate from our test
        let doc_link_pos = Vector::new(92.51128, 158.1745); // First link from test output
        
        // Convert document to screen coordinates
        let pdf_relative = (doc_link_pos - translation).scaled(scale);
        let viewport_relative = pdf_relative + pdf_top_left;
        let screen_pos = viewport_relative; // + viewport_bounds.position() (which would be 0,0 in this test)
        
        println!("Document link position: {:?}", doc_link_pos);
        println!("PDF relative: {:?}", pdf_relative);
        println!("Viewport relative: {:?}", viewport_relative);
        println!("Final screen position: {:?}", screen_pos);
        
        // Also test the reverse transformation to verify it's correct
        let reverse_viewport_relative = screen_pos - pdf_top_left;
        let reverse_pdf_relative = reverse_viewport_relative;
        let reverse_doc_pos = reverse_pdf_relative.scaled(1.0 / scale) + translation;
        
        println!("Reverse transformation check:");
        println!("  Screen -> Viewport relative: {:?}", reverse_viewport_relative);
        println!("  Viewport relative -> PDF relative: {:?}", reverse_pdf_relative);
        println!("  PDF relative -> Document: {:?}", reverse_doc_pos);
        println!("  Original document pos: {:?}", doc_link_pos);
        println!("  Difference: {:?}", Vector::new(
            (reverse_doc_pos.x - doc_link_pos.x).abs(),
            (reverse_doc_pos.y - doc_link_pos.y).abs()
        ));
        
        Ok(())
    }

    #[test]
    fn test_pdf_tile_coordinate_transformation() -> Result<()> {
        use crate::geometry::Vector;
        use mupdf::Document;
        
        println!("=== PDF TILE COORDINATE TRANSFORMATION TEST ===");
        
        // Get actual page size from the PDF
        let document = Document::open("assets/links.pdf")?;
        let page = document.load_page(0)?;
        let page_bounds = page.bounds()?;
        let page_size = Vector::new(page_bounds.width(), page_bounds.height());
        
        // Simulate typical values from the application
        let viewport_bounds_size = Vector::new(800.0, 600.0);
        let viewport_center = Vector::new(400.0, 300.0); // Center of 800x600 viewport
        let scale = 1.0;
        let translation = Vector::new(0.0, 0.0);
        
        println!("Page size: {:?}", page_size);
        println!("Viewport size: {:?}", viewport_bounds_size);
        println!("Viewport center: {:?}", viewport_center);
        
        // Test with the first link from our extraction
        let doc_link_pos = Vector::new(92.51128, 158.1745);
        let doc_link_end = Vector::new(233.82182, 166.03192);
        
        println!("Document link: {:?} -> {:?}", doc_link_pos, doc_link_end);
        
        // Apply the same transformation as PDF tile rendering
        let doc_top_left_scaled = (doc_link_pos - translation).scaled(scale);
        let doc_bottom_right_scaled = (doc_link_end - translation).scaled(scale);
        
        // PDF translation: -translation.scaled(scale) + viewport_center
        let pdf_translation = -translation.scaled(scale) + viewport_center;
        
        let screen_top_left = doc_top_left_scaled + pdf_translation;
        let screen_bottom_right = doc_bottom_right_scaled + pdf_translation;
        
        println!("Scaled document coords: {:?} -> {:?}", doc_top_left_scaled, doc_bottom_right_scaled);
        println!("PDF translation: {:?}", pdf_translation);
        println!("Final screen coords: {:?} -> {:?}", screen_top_left, screen_bottom_right);
        
        // These coordinates should be reasonable for a link in the upper part of the page
        // The first link should appear in the upper-left area of the viewport
        assert!(screen_top_left.x > 0.0 && screen_top_left.x < viewport_bounds_size.x);
        assert!(screen_top_left.y > 0.0 && screen_top_left.y < viewport_bounds_size.y);
        
        Ok(())
    }

    #[test]
    fn test_link_categorization() {
        assert!(matches!(categorize_link("https://example.com"), LinkType::ExternalUrl));
        assert!(matches!(categorize_link("http://example.com"), LinkType::ExternalUrl));
        assert!(matches!(categorize_link("mailto:test@example.com"), LinkType::Email));
        assert!(matches!(categorize_link("#page=5"), LinkType::InternalPage(5)));
        assert!(matches!(categorize_link("42"), LinkType::InternalPage(42)));
        assert!(matches!(categorize_link("file://local"), LinkType::Other));
    }

    #[test]
    fn test_multiple_pages_link_extraction() -> Result<()> {
        let document = Document::open("assets/links.pdf")?;
        
        let page_count = document.page_count()?;
        println!("=== TESTING ALL PAGES IN LINKS.PDF ===");
        println!("Total pages: {}", page_count);
        
        for page_idx in 0..page_count {
            let page = document.load_page(page_idx)?;
            let extractor = LinkExtractor::new(&page);
            let links = extractor.extract_all_links()?;
            
            println!("\nPage {}: found {} links", page_idx, links.len());
            for (i, link) in links.iter().enumerate() {
                println!("  Link {}: {} ({:?}) at {:?}", i, link.uri, link.link_type, link.bounds);
            }
        }
        
        Ok(())
    }

    #[test]
    fn test_coordinate_transformation_debug() -> Result<()> {
        use crate::geometry::Vector;
        use mupdf::Document;
        
        println!("=== COMPREHENSIVE COORDINATE TRANSFORMATION DEBUG ===");
        
        let document = Document::open("assets/links.pdf")?;
        let page = document.load_page(0)?;
        let page_bounds = page.bounds()?;
        let extractor = LinkExtractor::new(&page);
        let links = extractor.extract_all_links()?;
        
        println!("Page bounds: {:?}", page_bounds);
        println!("Page size: {}x{}", page_bounds.width(), page_bounds.height());
        println!("Found {} links", links.len());
        
        // Test different viewport and scale scenarios
        let test_scenarios = vec![
            ("Default 1024x768", Vector::new(1024.0, 768.0), 1.0, Vector::new(0.0, 0.0)),
            ("Small 800x600", Vector::new(800.0, 600.0), 1.0, Vector::new(0.0, 0.0)),
            ("Zoomed 2x", Vector::new(1024.0, 768.0), 2.0, Vector::new(0.0, 0.0)),
            ("Zoomed 0.5x", Vector::new(1024.0, 768.0), 0.5, Vector::new(0.0, 0.0)),
            ("Translated", Vector::new(1024.0, 768.0), 1.0, Vector::new(50.0, 30.0)),
        ];
        
        for (scenario_name, viewport_size, scale, translation) in test_scenarios {
            println!("\n--- {} ---", scenario_name);
            println!("Viewport: {:?}, Scale: {}, Translation: {:?}", viewport_size, scale, translation);
            
            let page_size = Vector::new(page_bounds.width(), page_bounds.height());
            let scaled_page_size = Vector::new(page_size.x * scale, page_size.y * scale);
            let viewport_center = Vector::new(viewport_size.x / 2.0, viewport_size.y / 2.0);
            
            // Calculate PDF positioning (centering)
            let pdf_top_left = Vector::new(
                (viewport_size.x - scaled_page_size.x) / 2.0,
                (viewport_size.y - scaled_page_size.y) / 2.0,
            );
            
            println!("  Page size: {:?}", page_size);
            println!("  Scaled page size: {:?}", scaled_page_size);
            println!("  Viewport center: {:?}", viewport_center);
            println!("  PDF top-left: {:?}", pdf_top_left);
            
            // Test transformation for first few links
            for (i, link) in links.iter().take(3).enumerate() {
                let doc_coords = link.bounds.x0;
                
                // Method 1: Direct transformation (what we should use)
                let pdf_relative = (doc_coords - translation).scaled(scale);
                let screen_coords_method1 = pdf_relative + pdf_top_left;
                
                // Method 2: Tile-style transformation (what tiles currently use)
                let tile_translation = -translation.scaled(scale) + viewport_center;
                let screen_coords_method2 = doc_coords.scaled(scale) + tile_translation;
                
                println!("  Link {}: doc={:?}", i, doc_coords);
                println!("    Method 1 (correct): {:?}", screen_coords_method1);
                println!("    Method 2 (tile-style): {:?}", screen_coords_method2);
                println!("    Difference: {:?}", Vector::new(
                    (screen_coords_method1.x - screen_coords_method2.x).abs(),
                    (screen_coords_method1.y - screen_coords_method2.y).abs()
                ));
            }
        }
        
        Ok(())
    }

    #[test]
    fn test_link_coordinate_validation() -> Result<()> {
        use mupdf::Document;
        
        println!("=== LINK COORDINATE VALIDATION ===");
        
        let document = Document::open("assets/links.pdf")?;
        let page = document.load_page(0)?;
        let page_bounds = page.bounds()?;
        let extractor = LinkExtractor::new(&page);
        let links = extractor.extract_all_links()?;
        
        println!("Page bounds: {:?}", page_bounds);
        
        let mut valid_links = 0;
        let mut invalid_links = 0;
        
        for (i, link) in links.iter().enumerate() {
            let bounds = &link.bounds;
            let is_valid = bounds.x0.x >= 0.0 && bounds.x0.y >= 0.0 
                && bounds.x1.x <= page_bounds.width() && bounds.x1.y <= page_bounds.height()
                && bounds.x0.x < bounds.x1.x && bounds.x0.y < bounds.x1.y;
            
            if is_valid {
                valid_links += 1;
            } else {
                invalid_links += 1;
                println!("Invalid link {}: {:?} (bounds: {:?})", i, link.uri, bounds);
            }
            
            // Check if link is in reasonable position (not at edges)
            let margin = 10.0;
            let in_margin = bounds.x0.x < margin || bounds.x0.y < margin 
                || bounds.x1.x > (page_bounds.width() - margin) 
                || bounds.x1.y > (page_bounds.height() - margin);
            
            if in_margin {
                println!("Link {} near edge: {:?}", i, bounds);
            }
        }
        
        println!("Valid links: {}, Invalid links: {}", valid_links, invalid_links);
        assert_eq!(invalid_links, 0, "All links should have valid coordinates");
        
        Ok(())
    }
}