use std::path::PathBuf;

use iced::{
    Border, Element, Length, Subscription, alignment,
    border::Radius,
    widget::{self, button, text, text_input},
};
use iced_aw::{Menu, menu::primary, menu_items, style::Status};
use iced_aw::{
    menu::{self, Item},
    menu_bar,
};
use serde::{Deserialize, Serialize};

use crate::pdf::{PdfMessage, PdfViewer};

#[derive(Debug, Default)]
pub struct App {
    pub pdfs: Vec<PdfViewer>,
    pub pdf_idx: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AppMessage {
    OpenFile(PathBuf),
    Debug(String),
    PdfMessage(PdfMessage),
}

impl App {
    pub fn new() -> Self {
        Self {
            pdfs: vec![PdfViewer::new()],
            ..Default::default()
        }
    }
    pub fn update(&mut self, message: AppMessage) -> iced::Task<AppMessage> {
        match message {
            AppMessage::OpenFile(path_buf) => self.pdfs[self.pdf_idx]
                .update(PdfMessage::OpenFile(path_buf))
                .map(AppMessage::PdfMessage),
            AppMessage::Debug(s) => {
                println!("[DEBUG] {s}");
                iced::Task::none()
            }
            AppMessage::PdfMessage(msg) => self.pdfs[self.pdf_idx]
                .update(msg)
                .map(AppMessage::PdfMessage),
        }
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

        let image = self.pdfs[self.pdf_idx].view().map(AppMessage::PdfMessage);

        let command_bar = text_input(":", "").width(Length::Fill);

        let c = widget::column![mb, image, command_bar];

        c.into()
    }

    pub fn subscription(&self) -> Subscription<AppMessage> {
        use iced::keyboard::{self, key};

        keyboard::on_key_press(|key, modifiers| {
            let move_step = 20.0;
            match key {
                key::Key::Character(c) => {
                    if c == "k" && modifiers.shift() {
                        Some(AppMessage::PdfMessage(PdfMessage::PreviousPage))
                    } else if c == "k" && !modifiers.shift() {
                        Some(AppMessage::PdfMessage(PdfMessage::MoveVertical(-move_step)))
                    } else if c == "j" && modifiers.shift() {
                        Some(AppMessage::PdfMessage(PdfMessage::NextPage))
                    } else if c == "j" && !modifiers.shift() {
                        Some(AppMessage::PdfMessage(PdfMessage::MoveVertical(move_step)))
                    } else if c == "+" {
                        Some(AppMessage::PdfMessage(PdfMessage::ZoomIn))
                    } else if c == "-" && !modifiers.shift() {
                        Some(AppMessage::PdfMessage(PdfMessage::ZoomOut))
                    } else if c == "0" {
                        Some(AppMessage::PdfMessage(PdfMessage::ZoomHome))
                    } else if c == "-" && modifiers.shift() {
                        Some(AppMessage::PdfMessage(PdfMessage::ZoomFit))
                    } else if c == "h" {
                        Some(AppMessage::PdfMessage(PdfMessage::MoveHorizontal(
                            -move_step,
                        )))
                    } else if c == "l" {
                        Some(AppMessage::PdfMessage(PdfMessage::MoveHorizontal(
                            move_step,
                        )))
                    } else {
                        None
                    }
                }
                _ => None,
            }
        })
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
