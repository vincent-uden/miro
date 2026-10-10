//! Adds quad-point markup annotations (highlight, strike-out, underline,
//! squiggly) with contents and authors to `assets/links_commented.pdf`, and
//! back-fills missing authors on any existing annotations.
//!
//! Run with `cargo run --example add_quad_annotations`.
use anyhow::{anyhow, Result};
use mupdf::{
    pdf::{PdfAnnotationType, PdfDocument, PdfPage},
    Quad, Rect, TextPageFlags,
};

const TEST_AUTHOR: &str = "TEST AUTHOR";

#[derive(Debug, Clone, Copy)]
enum QuadTarget {
    /// One quad from a single text line picked by prefix.
    Single(&'static str),
    /// One quad per line for each prefix.
    Multi(&'static [&'static str]),
}

const NEW_ANNOTATIONS: &[(PdfAnnotationType, QuadTarget, &str)] = &[
    (
        PdfAnnotationType::Highlight,
        QuadTarget::Single("This document contains"),
        "TEST QUAD COMMENT: the intro highlighted with a sticky-note style note.",
    ),
    (
        PdfAnnotationType::Highlight,
        QuadTarget::Multi(&["Check out Example Website", "Learn about Typst on GitHub"]),
        "TEST QUAD COMMENT: two bullet lines highlighted as separate quad points.",
    ),
    (
        PdfAnnotationType::Underline,
        QuadTarget::Single("Here are some external web links:"),
        "TEST QUAD COMMENT: underlined heading with a note.",
    ),
    (
        PdfAnnotationType::StrikeOut,
        QuadTarget::Single("1.2. Email Links"),
        "TEST QUAD COMMENT: struck-out heading with a note.",
    ),
    (
        PdfAnnotationType::Squiggly,
        QuadTarget::Single("Contact information:"),
        "TEST QUAD COMMENT: squiggly marked line with a note.",
    ),
];

fn text_lines(page: &PdfPage) -> Result<Vec<(String, Rect)>> {
    let text_page = page.to_text_page(TextPageFlags::empty())?;
    let mut lines = Vec::new();
    for block in text_page.blocks() {
        for line in block.lines() {
            let text: String = line.chars().filter_map(|ch| ch.char()).collect();
            lines.push((text, line.bounds()));
        }
    }
    Ok(lines)
}

fn quads_for_target(
    lines: &[(String, Rect)],
    target: QuadTarget,
) -> Result<Vec<Quad>> {
    match target {
        QuadTarget::Single(prefix) => {
            let (_, rect) = lines
                .iter()
                .find(|(text, _)| text.starts_with(prefix))
                .ok_or_else(|| anyhow!("text line not found: {prefix}"))?;
            Ok(vec![Quad::from(*rect)])
        }
        QuadTarget::Multi(prefixes) => prefixes
            .iter()
            .map(|prefix| {
                let (_, rect) = lines
                    .iter()
                    .find(|(text, _)| text.contains(prefix))
                    .ok_or_else(|| anyhow!("text line not found: {prefix}"))?;
                Ok(Quad::from(*rect))
            })
            .collect()
    }
}

fn main() -> Result<()> {
    let doc = PdfDocument::open("assets/links_commented.pdf")?;
    let mut pages: Vec<PdfPage> = doc
        .pages()?
        .flatten()
        .map(PdfPage::try_from)
        .collect::<Result<_, _>>()?;

    // Back-fill authors on pre-existing annotations.
    for (page_idx, page) in pages.iter_mut().enumerate() {
        for mut ann in page.annotations() {
            if ann
                .author()?
                .is_none_or(|author| author.trim().is_empty())
            {
                ann.set_author(TEST_AUTHOR)?;
                ann.update()?;
                eprintln!("page {page_idx}: back-filled author on existing annotation");
            }
        }
    }

    // Add the quad-point markup annotations on page 0.
    {
        let page = &mut pages[0];
        let lines = text_lines(page)?;
        for (subtype, target, contents) in NEW_ANNOTATIONS {
            let quads = quads_for_target(&lines, *target)?;
            let quad_count = quads.len();

            let mut ann = page.create_annotation(*subtype)?;
            // The rect of quad-point annotations is derived from the quads by
            // MuPDF, so it is not set explicitly.
            ann.set_quad_points(quads)?;
            ann.set_author(TEST_AUTHOR)?;
            ann.set_contents(contents)?;
            ann.update()?;
            eprintln!("page 0: added {subtype:?} with {quad_count} quads");
        }
        page.update()?;
    }

    // Save non-incrementally to a temp file, then move it into place.
    doc.save("assets/links_commented.tmp.pdf")?;
    drop(pages);
    drop(doc);
    std::fs::rename("assets/links_commented.tmp.pdf", "assets/links_commented.pdf")?;

    // Verify by reading the result back.
    let doc = PdfDocument::open("assets/links_commented.pdf")?;
    for (page_idx, page) in doc.pages()?.flatten().enumerate() {
        let page = PdfPage::try_from(page)?;
        for ann in page.annotations() {
            let subtype = ann.r#type()?;
            let author = ann.author()?.unwrap_or("");
            let contents = ann.contents()?.unwrap_or("");
            let quad_count = ann.quad_count()?;
            eprintln!(
                "page {page_idx} type {subtype:?} author {author:?} quads {quad_count} content {:?}",
                contents.chars().take(60).collect::<String>()
            );
            assert!(!author.trim().is_empty(), "annotation has no author");
        }
    }
    Ok(())
}
