use anyhow::Result;
use iced::widget::vertical_space;
use std::path::PathBuf;

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
    pub label: String,
    doc: Option<Document>,
    pub cur_page_idx: i32,
    pub n_pages: i32,
    nxt_page: Option<Page>,
    cur_page: Option<Page>,
    prv_page: Option<Page>,
    scale: f32,
    translation: Vector<f32>, // In document space
    pub invert_colors: bool,
    inner_state: inner::State,
    last_mouse_pos: Option<Vector<f32>>,
    panning: bool,
}

impl PdfViewer {
    pub fn new() -> Self {
        Self {
            scale: 1.0,
            ..Default::default()
        }
    }

    pub fn update(&mut self, message: PdfMessage) -> iced::Task<PdfMessage> {
        self.label = format!("{} {}/{}", self.name, self.cur_page_idx + 1, self.n_pages);
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
            PdfMessage::ZoomFit => {
                self.scale = self.zoom_fit_ratio().unwrap_or(1.0);
            }
            PdfMessage::MoveHorizontal(delta) => {
                self.translation.x += delta / self.scale;
            }
            PdfMessage::MoveVertical(delta) => {
                self.translation.y += delta / self.scale;
            }
            PdfMessage::UpdateBounds(rectangle) => {
                self.inner_state.bounds = rectangle;
            }
            PdfMessage::None => {}
            PdfMessage::MouseMoved(vector) => {
                if self.inner_state.bounds.contains(vector) {
                    if self.panning && self.last_mouse_pos.is_some() {
                        self.translation +=
                            (self.last_mouse_pos.unwrap() - vector).scaled(1.0 / self.scale);
                    }
                    self.last_mouse_pos = Some(vector);
                } else {
                    self.last_mouse_pos = None;
                }
            }
            PdfMessage::MouseLeftDown => {
                self.panning = true;
            }
            PdfMessage::MouseRightDown => {}
            PdfMessage::MouseLeftUp => {
                self.panning = false;
            }
            PdfMessage::MouseRightUp => {}
            PdfMessage::DebugPrintImage => {
                if let Some(page) = &self.cur_page {
                    let mut viewer = PageViewer::new(page, &self.inner_state)
                        .translation(self.translation)
                        .scale(self.scale)
                        .invert_colors(self.invert_colors);
                    viewer.debug_write("./debug.png");
                }
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
                .invert_colors(self.invert_colors)
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
        self.n_pages = doc.page_count()?;
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
        self.n_pages = doc.page_count()?;
        self.doc = Some(doc);
        self.set_page(self.cur_page_idx)?;
        Ok(())
    }

    fn zoom_fit_ratio(&mut self) -> Result<f32> {
        if let Some(page) = &self.cur_page {
            let page_size = page.bounds()?;
            let vertical_scale = self.inner_state.bounds.height() / page_size.height();
            let horizontal_scale = self.inner_state.bounds.width() / page_size.width();
            Ok(vertical_scale.min(horizontal_scale))
        } else {
            Ok(1.0)
        }
    }
}
