use std::path::{Path, PathBuf};

use iced::{
    Border, Element, Length,
    advanced::image,
    alignment,
    border::Radius,
    widget::{self, button, text, vertical_space},
};
use iced_aw::{Menu, menu::primary, menu_items, style::Status};
use iced_aw::{
    menu::{self, Item},
    menu_bar,
};
use mupdf::{Colorspace, Document, Matrix};
use serde::{Deserialize, Serialize};

use crate::pdf::PdfViewer;

#[derive(Debug, Default)]
pub struct App {
    img_handle: Option<image::Handle>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AppMessage {
    OpenFile(PathBuf),
    Debug(String),
}

impl App {
    pub fn update(&mut self, message: AppMessage) -> iced::Task<AppMessage> {
        match message {
            AppMessage::OpenFile(path_buf) => {
                self.load_file(&path_buf);
            }
            AppMessage::Debug(s) => {
                format!("[DEBUG] {s}");
            }
        }
        iced::Task::none()
    }

    pub fn view(&self) -> iced::Element<'_, AppMessage> {
        let menu_tpl_1 = |items| Menu::new(items).max_width(180.0).offset(0.0).spacing(5.0);

        #[rustfmt::skip]
        let mb = menu_bar!(
            (debug_button_s("File"), menu_tpl_1(menu_items!(
                (debug_button("New"))
                (debug_button("Open"))
            )))
            (debug_button_s("Edit"), menu_tpl_1(menu_items!(
                (debug_button("Undo"))
                (debug_button("Redo"))
            )))
        ).draw_path(menu::DrawPath::Backdrop)
            .style(|theme:&iced::Theme, status: Status | menu::Style{
            path_border: Border{
                radius: Radius::new(6.0),
                ..Default::default()
            },
            ..primary(theme, status)
        });

        let image: Element<'_, AppMessage> = if let Some(h) = &self.img_handle {
            PdfViewer::<image::Handle>::new(h).into()
        } else {
            vertical_space().into()
        };

        let c = widget::column![mb, vertical_space(), image];

        c.into()
    }

    fn load_file(&mut self, path: &Path) {
        let doc = Document::open(path.to_str().unwrap()).unwrap();
        let page = doc.load_page(0).unwrap();
        let matrix = Matrix::default();
        let pixmap = page
            .to_pixmap(&matrix, &Colorspace::device_rgb(), 1.0, false)
            .unwrap();
        let image = pixmap.pixels().unwrap();
        let image_data: Vec<_> = image.iter().flat_map(|&num| num.to_le_bytes()).collect();
        self.img_handle = Some(image::Handle::from_rgba(
            pixmap.width(),
            pixmap.height(),
            image_data,
        ));
    }
}

fn base_button<'a>(
    content: impl Into<Element<'a, AppMessage>>,
    msg: AppMessage,
) -> button::Button<'a, AppMessage> {
    button(content)
        .padding([4, 8])
        .style(iced::widget::button::primary)
        .on_press(msg)
}

fn labeled_button(
    label: &str,
    msg: AppMessage,
) -> button::Button<AppMessage, iced::Theme, iced::Renderer> {
    base_button(text(label).align_y(alignment::Vertical::Center), msg)
}

fn debug_button(label: &str) -> button::Button<AppMessage, iced::Theme, iced::Renderer> {
    labeled_button(label, AppMessage::Debug(label.into())).width(Length::Fill)
}

fn debug_button_s(label: &str) -> button::Button<AppMessage, iced::Theme, iced::Renderer> {
    labeled_button(label, AppMessage::Debug(label.into())).width(Length::Shrink)
}
