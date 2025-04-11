use std::{fs::canonicalize, path::PathBuf};

use iced::{
    Background, Border, Element, Length, Padding, Shadow, Subscription, Theme, alignment,
    border::{self, Radius},
    theme::palette,
    widget::{
        self, button, container, scrollable,
        scrollable::{Direction, Scrollbar},
        text, vertical_space,
    },
};
use iced_aw::{Menu, iced_fonts::REQUIRED_FONT, menu::primary, menu_items};
use iced_aw::{
    menu::{self, Item},
    menu_bar,
};
use iced_fonts::required::{RequiredIcons, icon_to_string};
use rfd::FileDialog;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::debug;

use crate::{APP_KEYMAP, pdf::PdfMessage};
use crate::{
    pdf::widget::PdfViewer,
    watch::{WatchMessage, WatchNotification, file_watcher},
};

#[derive(Debug, Default)]
pub struct App {
    pub pdfs: Vec<PdfViewer>,
    pub pdf_idx: usize,
    pub file_watcher: Option<mpsc::Sender<WatchMessage>>,
    pub dark_mode: bool,
    pub invert_pdf: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AppMessage {
    OpenFile(PathBuf),
    OpenNewFileFinder,
    Debug(String),
    PdfMessage(PdfMessage),
    OpenTab(usize),
    CloseTab(usize),
    PreviousTab,
    NextTab,
    #[serde(skip)]
    FileWatcher(WatchNotification),
    ToggleDarkMode,
    InvertPdf,
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
                let path_buf = canonicalize(path_buf).unwrap();
                if self.pdfs.is_empty() {
                    self.pdfs.push(PdfViewer::new());
                    self.pdf_idx = 0;
                }
                if let Some(sender) = &self.file_watcher {
                    // We should never fill this up from here
                    let _ = sender.blocking_send(WatchMessage::StartWatch(path_buf.clone()));
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
            AppMessage::OpenNewFileFinder => {
                if let Some(path_buf) = FileDialog::new().add_filter("Pdf", &["pdf"]).pick_file() {
                    self.pdfs.push(PdfViewer::new());
                    self.pdf_idx = self.pdfs.len() - 1;
                    iced::Task::done(AppMessage::OpenFile(path_buf))
                } else {
                    iced::Task::none()
                }
            }
            AppMessage::CloseTab(i) => {
                if let Some(sender) = &self.file_watcher {
                    // We should never fill this up from here
                    let _ = sender.blocking_send(WatchMessage::StopWatch(
                        self.pdfs[self.pdf_idx].path.clone(),
                    ));
                }
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
            AppMessage::FileWatcher(watch_notification) => {
                match watch_notification {
                    WatchNotification::Ready(sender) => {
                        self.file_watcher = Some(sender);
                    }
                    WatchNotification::Changed(path_buf) => {
                        for pdf in &mut self.pdfs {
                            if pdf.path == path_buf {
                                let _ = pdf.update(PdfMessage::RefreshFile);
                            }
                        }
                    }
                }
                iced::Task::none()
            }
            AppMessage::ToggleDarkMode => {
                self.dark_mode = !self.dark_mode;
                iced::Task::none()
            }
            AppMessage::InvertPdf => {
                self.invert_pdf = !self.invert_pdf;
                for pdf in &mut self.pdfs {
                    pdf.invert_colors = self.invert_pdf;
                }
                iced::Task::none()
            }
        }
    }

    pub fn view(&self) -> iced::Element<'_, AppMessage> {
        let menu_tpl_1 = |items| Menu::new(items).max_width(180.0).offset(0.0).spacing(0.0);

        let mb = container(
            menu_bar!((
                debug_button_s("File"),
                menu_tpl_1(menu_items!((menu_button(
                    "Open",
                    AppMessage::OpenNewFileFinder
                ))(menu_button(
                    "Close",
                    AppMessage::CloseTab(self.pdf_idx)
                ))))
            )(
                debug_button_s("View"),
                menu_tpl_1(menu_items!((menu_button(
                    if self.dark_mode {
                        "Light Interface"
                    } else {
                        "Dark Interface"
                    },
                    AppMessage::ToggleDarkMode
                ))(menu_button(
                    if self.invert_pdf {
                        "Light Pdf"
                    } else {
                        "Dark Pdf"
                    },
                    AppMessage::InvertPdf
                ))))
            ))
            .draw_path(menu::DrawPath::Backdrop)
            .style(
                |theme: &iced::Theme, status: iced_aw::style::Status| menu::Style {
                    menu_background_expand: 0.0.into(),
                    bar_background_expand: 0.0.into(),
                    bar_background: Background::Color(
                        theme.extended_palette().secondary.base.color,
                    ),
                    menu_border: Border {
                        radius: Radius::new(0.0),
                        ..Default::default()
                    },
                    ..primary(theme, status)
                },
            ),
        )
        .width(Length::Fill)
        .style(|theme| container::Style {
            background: Some(Background::Color(
                theme.extended_palette().secondary.base.color,
            )),
            ..Default::default()
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
        command_bar = command_bar.spacing(4.0).height(Length::Shrink);
        let tabs = scrollable(command_bar).direction(Direction::Horizontal(
            Scrollbar::default().scroller_width(0.0).width(0.0),
        ));

        let c = widget::column![mb, image, tabs];

        c.into()
    }

    pub fn subscription(&self) -> Subscription<AppMessage> {
        use iced::keyboard::{self};
        let keys = keyboard::on_key_press(|key, modifiers| {
            let key_map = APP_KEYMAP.read().unwrap();
            key_map.event(key, modifiers)
        });

        Subscription::batch(vec![
            keys,
            Subscription::run(file_watcher).map(AppMessage::FileWatcher),
        ])
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
    labeled_button(label, AppMessage::Debug(label.into()))
        .width(Length::Shrink)
        .style(move |theme, status| {
            let palette = theme.extended_palette();
            let pair = match status {
                button::Status::Active => palette.secondary.base,
                button::Status::Hovered => palette.secondary.weak,
                button::Status::Pressed => palette.secondary.strong,
                button::Status::Disabled => palette.secondary.weak,
            };
            button::Style {
                text_color: pair.text,
                background: Some(Background::Color(pair.color)),
                ..Default::default()
            }
        })
}

fn menu_button(
    label: &str,
    msg: AppMessage,
) -> button::Button<AppMessage, iced::Theme, iced::Renderer> {
    base_button(text(label), msg)
        .width(Length::Fill)
        .style(move |theme, status| {
            let palette = theme.extended_palette();
            let pair = match status {
                button::Status::Active => palette.background.base,
                button::Status::Hovered => palette.background.weak,
                button::Status::Pressed => palette.background.strong,
                button::Status::Disabled => palette.secondary.weak,
            };
            button::Style {
                text_color: pair.text,
                background: Some(Background::Color(pair.color)),
                ..Default::default()
            }
        })
        .into()
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
