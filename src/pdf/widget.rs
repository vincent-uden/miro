use anyhow::{Result, anyhow};
use iced::widget::vertical_space;
use std::path::{Path, PathBuf};

use mupdf::{Document, Page};

use crate::geometry::Vector;

use super::{
    PdfMessage,
    inner::{self, PageViewer},
};

#[derive(Debug, Default)]
pub struct PdfViewer {
    pub name: String,
    pub path: PathBuf,
    doc: Option<Document>,
    cur_page_idx: i32,
    nxt_page: Option<Page>,
    cur_page: Option<Page>,
    prv_page: Option<Page>,
    scale: f32,
    translation: Vector<f32>, // In document space
    inner_state: inner::State,
}

impl PdfViewer {
    pub fn new() -> Self {
        Self {
            scale: 1.0,
            ..Default::default()
        }
    }

    pub fn update(&mut self, message: PdfMessage) -> iced::Task<PdfMessage> {
        match message {
            PdfMessage::OpenFile(path_buf) => self.load_file(path_buf).unwrap(),
            PdfMessage::RefreshFile => self.refresh_file().unwrap(),
            PdfMessage::NextPage => self.set_page(self.cur_page_idx + 1).unwrap(),
            PdfMessage::PreviousPage => self.set_page(self.cur_page_idx - 1).unwrap(),
            PdfMessage::ZoomIn => {
                self.scale *= 1.2;
            }
            PdfMessage::ZoomOut => {
                self.scale /= 1.2;
            }
            PdfMessage::ZoomHome => {
                self.scale = 1.0;
            }
            PdfMessage::ZoomFit => todo!(),
            PdfMessage::MoveHorizontal(delta) => {
                self.translation.x += delta / self.scale;
            }
            PdfMessage::MoveVertical(delta) => {
                self.translation.y += delta / self.scale;
            }
            PdfMessage::UpdateBounds(rectangle) => {
                self.inner_state.bounds = rectangle;
            }
        }
        iced::Task::none()
    }

    pub fn view(&self) -> iced::Element<'_, PdfMessage> {
        // TODO: Next and previous pages
        if let Some(p) = &self.cur_page {
            PageViewer::new(p, &self.inner_state)
                .translation(self.translation)
                .scale(self.scale)
                .into()
        } else {
            vertical_space().into()
        }
    }

    fn set_page(&mut self, idx: i32) -> Result<()> {
        if let Some(doc) = &self.doc {
            self.cur_page_idx = idx.clamp(0, doc.page_count()?);
            self.prv_page = doc.load_page(self.cur_page_idx - 1).ok();
            self.cur_page = doc.load_page(self.cur_page_idx).ok();
            self.nxt_page = doc.load_page(self.cur_page_idx + 1).ok();
        }
        Ok(())
    }

    fn load_file(&mut self, path: PathBuf) -> Result<()> {
        let doc = Document::open(path.to_str().unwrap())?;
        self.doc = Some(doc);
        self.name = path
            .file_name()
            .expect("The pdf must have a file name")
            .to_string_lossy()
            .to_string();
        self.path = path.to_path_buf();
        self.set_page(0)?;
        Ok(())
    }

    fn refresh_file(&mut self) -> Result<()> {
        let doc = Document::open(self.path.to_str().unwrap())?;
        self.doc = Some(doc);
        self.set_page(self.cur_page_idx)?;
        Ok(())
    }
}
