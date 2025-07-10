use anyhow::Result;
use mupdf::{Page, Rect as MupdfRect};

#[derive(Debug, Clone)]
pub struct ClickableLink {
    pub bounds: MupdfRect,
    pub uri: String,
    pub page: u32,
    pub is_external: bool,
}

#[derive(Debug)]
pub struct LinkDetector<'a> {
    page: &'a Page,
}

impl<'a> LinkDetector<'a> {
    pub fn new(page: &'a Page) -> Self {
        Self { page }
    }

    pub fn get_all_links(&self) -> Result<Vec<ClickableLink>> {
        let links_iter = self.page.links()?;
        let mut clickable_links = Vec::new();

        for link in links_iter {
            let is_external = link.uri.starts_with("http://") 
                || link.uri.starts_with("https://") 
                || link.uri.starts_with("mailto:");

            clickable_links.push(ClickableLink {
                bounds: link.bounds,
                uri: link.uri,
                page: link.page,
                is_external,
            });
        }

        Ok(clickable_links)
    }

    pub fn find_link_at_point(&self, x: f32, y: f32) -> Result<Option<ClickableLink>> {
        let links = self.get_all_links()?;
        
        for link in links {
            if point_in_rect(x, y, link.bounds) {
                return Ok(Some(link));
            }
        }

        Ok(None)
    }

    pub fn find_links_in_rect(&self, rect: MupdfRect) -> Result<Vec<ClickableLink>> {
        let links = self.get_all_links()?;
        let mut intersecting_links = Vec::new();

        for link in links {
            if rectangles_intersect(rect, link.bounds) {
                intersecting_links.push(link);
            }
        }

        Ok(intersecting_links)
    }
}

fn point_in_rect(x: f32, y: f32, rect: MupdfRect) -> bool {
    x >= rect.x0 && x <= rect.x1 && y >= rect.y0 && y <= rect.y1
}

fn rectangles_intersect(rect1: MupdfRect, rect2: MupdfRect) -> bool {
    rect1.x0 < rect2.x1 && rect1.x1 > rect2.x0 && rect1.y0 < rect2.y1 && rect1.y1 > rect2.y0
}

#[cfg(test)]
mod tests {
    use super::*;
    use mupdf::Document;

    #[test]
    fn test_link_detection_basic() -> Result<()> {
        let document = Document::open("assets/links.pdf")?;
        let page = document.load_page(0)?;
        let detector = LinkDetector::new(&page);

        let links = detector.get_all_links()?;
        println!("Found {} links", links.len());
        
        for (i, link) in links.iter().enumerate() {
            println!("Link {}: {} at {:?}", i, link.uri, link.bounds);
        }

        Ok(())
    }

    #[test]
    fn test_point_intersection() -> Result<()> {
        let document = Document::open("assets/links.pdf")?;
        let page = document.load_page(0)?;
        let detector = LinkDetector::new(&page);

        let links = detector.get_all_links()?;
        
        if !links.is_empty() {
            let first_link = &links[0];
            let center_x = (first_link.bounds.x0 + first_link.bounds.x1) / 2.0;
            let center_y = (first_link.bounds.y0 + first_link.bounds.y1) / 2.0;
            
            let found_link = detector.find_link_at_point(center_x, center_y)?;
            assert!(found_link.is_some());
            assert_eq!(found_link.unwrap().uri, first_link.uri);
        }

        Ok(())
    }

    #[test]
    fn test_rect_intersection() -> Result<()> {
        let document = Document::open("assets/links.pdf")?;
        let page = document.load_page(0)?;
        let detector = LinkDetector::new(&page);

        let search_rect = MupdfRect {
            x0: 0.0,
            y0: 0.0,
            x1: 1000.0,
            y1: 1000.0,
        };

        let links_in_rect = detector.find_links_in_rect(search_rect)?;
        let all_links = detector.get_all_links()?;
        
        assert_eq!(links_in_rect.len(), all_links.len());

        Ok(())
    }

    #[test]
    fn test_external_link_detection() -> Result<()> {
        let document = Document::open("assets/links.pdf")?;
        let page = document.load_page(0)?;
        let detector = LinkDetector::new(&page);

        let links = detector.get_all_links()?;
        
        for link in &links {
            println!("Link: {} (external: {})", link.uri, link.is_external);
            
            if link.uri.starts_with("http") {
                assert!(link.is_external);
            }
        }

        Ok(())
    }

    #[test]
    fn test_no_links_document() -> Result<()> {
        let document = Document::open("assets/text-copy-test.pdf")?;
        let page = document.load_page(0)?;
        let detector = LinkDetector::new(&page);

        let links = detector.get_all_links()?;
        println!("Found {} links in text-copy-test.pdf", links.len());

        let no_link = detector.find_link_at_point(100.0, 100.0)?;
        assert!(no_link.is_none());

        Ok(())
    }
}