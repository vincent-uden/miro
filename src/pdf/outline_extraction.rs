use anyhow::Result;
use mupdf::{Document, Outline};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutlineItem {
    pub title: String,
    pub page: Option<u32>,
    pub level: u32,
    pub children: Vec<OutlineItem>,
}

pub struct OutlineExtractor<'a> {
    document: &'a Document,
}

impl<'a> OutlineExtractor<'a> {
    pub fn new(document: &'a Document) -> Self {
        Self { document }
    }

    pub fn extract_outline(&self) -> Result<Vec<OutlineItem>> {
        match self.document.outlines() {
            Ok(outlines) => {
                if outlines.is_empty() {
                    Ok(Vec::new())
                } else {
                    let mut items = Vec::new();
                    for outline in &outlines {
                        let converted_items = convert_outline_recursive(outline, 0)?;
                        items.extend(converted_items);
                    }
                    Ok(items)
                }
            }
            Err(e) => Err(e.into()),
        }
    }
}

fn convert_outline_recursive(outline: &Outline, level: u32) -> Result<Vec<OutlineItem>> {
    let mut items = Vec::new();
    let mut current_item = OutlineItem {
        title: outline.title.clone(),
        page: outline.page,
        level,
        children: Vec::new(),
    };
    for child in &outline.down {
        let child_items = convert_outline_recursive(child, level + 1)?;
        current_item.children.extend(child_items);
    }
    items.push(current_item);
    Ok(items)
}
