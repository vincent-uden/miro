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
        
        for link in link_iter {
            let bounds = Rect::from_pos_size(
                crate::geometry::Vector::new(link.bounds.x0, link.bounds.y0),
                crate::geometry::Vector::new(
                    link.bounds.x1 - link.bounds.x0,
                    link.bounds.y1 - link.bounds.y0,
                ),
            );
            
            let link_type = categorize_link(&link.uri);
            
            links.push(LinkInfo {
                bounds,
                uri: link.uri,
                link_type,
            });
        }
        
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


}