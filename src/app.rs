use std::path::{Path, PathBuf};

use iced::{
    Border, Element, Length, Subscription,
    advanced::image,
    alignment,
    border::Radius,
    keyboard::on_key_press,
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
    doc: Option<Document>,
    page: i32,
    img_handle: Option<image::Handle>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AppMessage {
    OpenFile(PathBuf),
    Debug(String),
    NextPage,
    PreviousPage,
}

impl App {
    pub fn update(&mut self, message: AppMessage) -> iced::Task<AppMessage> {
        match message {
            AppMessage::OpenFile(path_buf) => {
                self.load_file(&path_buf);
            }
            AppMessage::Debug(s) => {
                println!("[DEBUG] {s}");
            }
            AppMessage::NextPage => {
                if self.page
                    < self
                        .doc
                        .as_ref()
                        .map(|d| d.page_count().unwrap_or(0))
                        .unwrap_or(0)
                        - 1
                {
                    self.page += 1;
                    self.show_current_page();
                }
            }
            AppMessage::PreviousPage => {
                if self.page > 0 {
                    self.page -= 1;
                    self.show_current_page();
                }
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

    pub fn subscription(&self) -> Subscription<AppMessage> {
        use iced::keyboard::{self, key};

        keyboard::on_key_press(|key, modifiers| match (key, modifiers) {
            (key::Key::Character(c), _) => {
                if c == "k" {
                    Some(AppMessage::PreviousPage)
                } else if c == "j" {
                    Some(AppMessage::NextPage)
                } else {
                    None
                }
            }
            _ => None,
        })
    }

    fn load_file(&mut self, path: &Path) {
        let doc = Document::open(path.to_str().unwrap()).unwrap();
        self.doc = Some(doc);
        self.page = 0;
        self.show_current_page();
    }

    fn show_current_page(&mut self) {
        if let Some(doc) = &self.doc {
            let page = doc.load_page(self.page).unwrap();
            let mut matrix = Matrix::default();
            matrix.scale(5.0, 5.0);
            let pixmap = page
                .to_pixmap(&matrix, &Colorspace::device_rgb(), 1.0, false)
                .unwrap();
            let mut image_data = pixmap.samples().to_vec();
            image_data.clone_from_slice(pixmap.samples());
            self.img_handle = Some(image::Handle::from_rgba(
                pixmap.width(),
                pixmap.height(),
                image_data,
            ));
        }
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
