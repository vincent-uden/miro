use std::path::PathBuf;

use iced::{
    Border, Element, Length, alignment,
    border::Radius,
    widget::{self, button, text, vertical_space},
};
use iced_aw::{Menu, menu::primary, menu_items, style::Status};
use iced_aw::{
    menu::{self, Item},
    menu_bar,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default)]
pub struct App {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AppMessage {
    OpenFile(PathBuf),
    Debug(String),
}

impl App {
    pub fn update(&mut self, message: AppMessage) -> iced::Task<AppMessage> {
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

        let c = widget::column![mb, vertical_space(),];

        c.into()
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
