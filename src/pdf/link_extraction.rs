use anyhow::Result;
use mupdf::{Link, Page};

use crate::geometry::{Rect, Vector};

#[derive(Debug, Clone)]
pub struct LinkInfo {
    /// The link bounds in document space
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
            let bounds = Rect::from_points(
                Vector::new(link.bounds.x0, link.bounds.y0),
                Vector::new(link.bounds.x1, link.bounds.y1),
            );
            let link_type = categorize_link(&link);
            links.push(LinkInfo {
                bounds,
                uri: link.uri,
                link_type,
            });
        }

        Ok(links)
    }
}

fn categorize_link(link: &Link) -> LinkType {
    if link.uri.starts_with("http://") || link.uri.starts_with("https://") {
        LinkType::ExternalUrl
    } else if link.uri.starts_with("mailto:") {
        LinkType::Email
    } else if link.uri.starts_with("#page=") || link.uri.starts_with("#nameddest=") {
        LinkType::InternalPage(link.page)
    } else if link.uri.chars().all(|c| c.is_ascii_digit()) {
        if let Ok(page_num) = link.uri.parse::<u32>() {
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
    use mupdf::{Document, Link, Rect as MupdfRect};

    fn create_mock_link(uri: &str, page: u32) -> Link {
        Link {
            bounds: MupdfRect {
                x0: 0.0,
                y0: 0.0,
                x1: 100.0,
                y1: 20.0,
            },
            uri: uri.to_string(),
            page,
        }
    }

    #[test]
    fn test_links_pdf_extraction() -> Result<()> {
        let document = Document::open("assets/links.pdf")?;
        let page = document.load_page(0)?;
        let extractor = LinkExtractor::new(&page);

        let links = extractor.extract_all_links()?;

        assert!(!links.is_empty(), "Should find links in the test document");

        let has_external = links
            .iter()
            .any(|l| matches!(l.link_type, LinkType::ExternalUrl));
        let has_email = links.iter().any(|l| matches!(l.link_type, LinkType::Email));

        assert!(has_external, "Should find external links");
        assert!(has_email, "Should find email links");

        Ok(())
    }

    #[test]
    fn test_link_categorization() {
        let external_link = create_mock_link("https://example.com", 0);
        assert!(matches!(
            categorize_link(&external_link),
            LinkType::ExternalUrl
        ));

        let http_link = create_mock_link("http://example.com", 0);
        assert!(matches!(categorize_link(&http_link), LinkType::ExternalUrl));

        let email_link = create_mock_link("mailto:test@example.com", 0);
        assert!(matches!(categorize_link(&email_link), LinkType::Email));

        let page_link = create_mock_link("#page=5", 5);
        assert!(matches!(
            categorize_link(&page_link),
            LinkType::InternalPage(5)
        ));

        let numeric_link = create_mock_link("42", 42);
        assert!(matches!(
            categorize_link(&numeric_link),
            LinkType::InternalPage(42)
        ));

        let other_link = create_mock_link("file://local", 0);
        assert!(matches!(categorize_link(&other_link), LinkType::Other));
    }
}
