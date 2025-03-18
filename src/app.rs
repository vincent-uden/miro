use std::path::PathBuf;

use iced::{
    Background, Border, Element, Length, Shadow, Subscription, Theme, alignment,
    border::{self, Radius},
    theme::palette,
    widget::{self, button, container, scrollable, text, vertical_space},
};
use iced_aw::{Menu, iced_fonts::REQUIRED_FONT, menu::primary, menu_items};
use iced_aw::{
    menu::{self, Item},
    menu_bar,
};
use iced_fonts::required::{RequiredIcons, icon_to_string};
use rfd::FileDialog;
use serde::{Deserialize, Serialize};

use crate::{
    APP_KEYMAP,
    pdf::{PdfMessage, PdfViewer},
};

#[derive(Debug, Default)]
pub struct App {
    pub pdfs: Vec<PdfViewer>,
    pub pdf_idx: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AppMessage {
    OpenFile(PathBuf),
    OpenNewFile,
    Debug(String),
    PdfMessage(PdfMessage),
    OpenTab(usize),
    CloseTab(usize),
    PreviousTab,
    NextTab,
}

impl App {
    pub fn new() -> Self {
        Self {
            pdfs: vec![],
            ..Default::default()
        }
    }
    pub fn update(&mut self, message: AppMessage) -> iced::Task<AppMessage> {
        match message {
            AppMessage::OpenFile(path_buf) => {
                if self.pdfs.is_empty() {
                    self.pdfs.push(PdfViewer::new());
                    self.pdf_idx = 0;
                }
                let out = self.pdfs[self.pdf_idx]
                    .update(PdfMessage::OpenFile(path_buf))
                    .map(AppMessage::PdfMessage);
                out
            }
            AppMessage::Debug(s) => {
                println!("[DEBUG] {s}");
                iced::Task::none()
            }
            AppMessage::PdfMessage(msg) => self.pdfs[self.pdf_idx]
                .update(msg)
                .map(AppMessage::PdfMessage),
            AppMessage::OpenTab(i) => {
                self.pdf_idx = i;
                iced::Task::none()
            }
            AppMessage::OpenNewFile => {
                if let Some(path_buf) = FileDialog::new().add_filter("Pdf", &["pdf"]).pick_file() {
                    self.pdfs.push(PdfViewer::new());
                    self.pdf_idx = self.pdfs.len() - 1;
                    iced::Task::done(AppMessage::OpenFile(path_buf))
                } else {
                    iced::Task::none()
                }
            }
            AppMessage::CloseTab(i) => {
                self.pdfs.remove(i);
                if self.pdf_idx >= self.pdfs.len() {
                    self.pdf_idx = self.pdfs.len() - 1;
                }
                iced::Task::none()
            }
            AppMessage::PreviousTab => {
                self.pdf_idx = (self.pdf_idx - 1).max(0);
                iced::Task::none()
            }
            AppMessage::NextTab => {
                self.pdf_idx = (self.pdf_idx + 1).min(self.pdfs.len() - 1);
                iced::Task::none()
            }
        }
    }

    pub fn view(&self) -> iced::Element<'_, AppMessage> {
        let menu_tpl_1 = |items| Menu::new(items).max_width(180.0).offset(0.0).spacing(5.0);

        #[rustfmt::skip]
        let mb = menu_bar!(
            (debug_button_s("File"), menu_tpl_1(menu_items!(
                (debug_button("New"))
                (menu_button("Open", AppMessage::OpenNewFile))
            )))
            (debug_button_s("Edit"), menu_tpl_1(menu_items!(
                (debug_button("Undo"))
                (debug_button("Redo"))
            )))
        ).draw_path(menu::DrawPath::Backdrop)
            .style(|theme:&iced::Theme, status: iced_aw::style::Status| menu::Style{
            path_border: Border{
                radius: Radius::new(6.0),
                ..Default::default()
            },
            ..primary(theme, status)
        });

        let image: Element<'_, AppMessage> = if !self.pdfs.is_empty() {
            self.pdfs[self.pdf_idx]
                .view()
                .map(AppMessage::PdfMessage)
                .into()
        } else {
            vertical_space().into()
        };

        let mut command_bar = widget::Row::new();
        for (i, pdf) in self.pdfs.iter().enumerate() {
            command_bar = command_bar.push(file_tab(
                &pdf.name,
                AppMessage::OpenTab(i),
                AppMessage::CloseTab(i),
                i == self.pdf_idx,
            ));
        }
        command_bar = command_bar.spacing(4.0);
        let tabs = scrollable(command_bar);

        let c = widget::column![mb, image, tabs];

        c.into()
    }

    pub fn subscription(&self) -> Subscription<AppMessage> {
        use iced::keyboard::{self};
        keyboard::on_key_press(|key, modifiers| {
            let key_map = APP_KEYMAP.read().unwrap();
            key_map.event(key, modifiers)
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

fn menu_button(
    label: &str,
    msg: AppMessage,
) -> button::Button<AppMessage, iced::Theme, iced::Renderer> {
    base_button(text(label), msg).width(Length::Fill)
}

fn file_tab(
    label: &str,
    on_press: AppMessage,
    on_close: AppMessage,
    is_open: bool,
) -> Element<'_, AppMessage> {
    container(
        widget::row![
            labeled_button(label, on_press).style(file_tab_style),
            // TODO: Fix alignment on the x, it doesnt look great next to the text
            base_button(
                text(icon_to_string(RequiredIcons::X))
                    .align_y(alignment::Vertical::Bottom)
                    .font(REQUIRED_FONT),
                on_close
            )
            .style(file_tab_style),
        ]
        .spacing(2.0),
    )
    .style(move |theme| {
        let palette = theme.extended_palette();
        let pair = if is_open {
            palette.secondary.strong
        } else {
            palette.secondary.base
        };
        container::Style {
            text_color: Some(pair.text),
            background: Some(Background::Color(pair.color)),
            border: border::rounded(border::top(4)),
            shadow: Shadow::default(),
        }
    })
    .into()
}

pub fn file_tab_style(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();
    let base = styled(palette.secondary.base);

    match status {
        button::Status::Active | button::Status::Pressed => button::Style {
            background: None,
            ..base
        },
        button::Status::Hovered => button::Style {
            background: None,
            ..base
        },
        button::Status::Disabled => disabled(base),
    }
}

fn styled(pair: palette::Pair) -> button::Style {
    button::Style {
        background: Some(Background::Color(pair.color)),
        text_color: pair.text,
        border: border::rounded(2),
        ..button::Style::default()
    }
}

fn disabled(style: button::Style) -> button::Style {
    button::Style {
        background: style
            .background
            .map(|background| background.scale_alpha(0.5)),
        text_color: style.text_color.scale_alpha(0.5),
        ..style
    }
}
