use std::{fs::canonicalize, path::PathBuf};

use iced::{
    Background, Border, Element, Event, Length, Shadow, Subscription, Theme, alignment,
    border::{self, Radius},
    event::listen_with,
    futures::{SinkExt, Stream},
    stream,
    theme::palette,
    widget::{
        self, PaneGrid, button, container, pane_grid, responsive, row, scrollable,
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
use keybinds::{KeySeq, Keybind};
use rfd::FileDialog;
use serde::{Deserialize, Serialize};
use strum::EnumString;
use tokio::sync::mpsc;
use tracing::debug;

use crate::{
    CONFIG, WORKER_RX,
    bookmarks::{BookmarkMessage, BookmarkStore},
    config::BindableMessage,
    geometry::Vector,
    pdf::{
        PdfMessage,
        cache::{WorkerCommand, WorkerResponse},
    },
    rpc::rpc_server,
};
use crate::{
    pdf::widget::PdfViewer,
    watch::{WatchMessage, WatchNotification, file_watcher},
};

#[derive(Debug)]
enum PaneType {
    Pdf,
    Sidebar,
}

#[derive(Debug)]
struct Pane {
    id: usize,
    pane_type: PaneType,
}

#[derive(Debug)]
pub struct App {
    pub pdfs: Vec<PdfViewer>,
    pub pdf_idx: usize,
    pub file_watcher: Option<mpsc::Sender<WatchMessage>>,
    pub dark_mode: bool,
    pub invert_pdf: bool,
    bookmark_store: BookmarkStore,
    command_tx: tokio::sync::mpsc::UnboundedSender<WorkerCommand>,
    pane_state: pane_grid::State<Pane>,
    pane_ratio: f32,
    sidebar_showing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, EnumString, Default)]
pub enum AppMessage {
    OpenFile(PathBuf),
    CloseFile(PathBuf),
    OpenNewFileFinder,
    Debug(String),
    PdfMessage(PdfMessage),
    OpenTab(usize),
    CloseTab(usize),
    PreviousTab,
    NextTab,
    #[strum(disabled)]
    #[serde(skip)]
    FileWatcher(WatchNotification),
    ToggleDarkModeUi,
    ToggleDarkModePdf,
    MouseMoved(Vector<f32>),
    MouseLeftDown,
    MouseRightDown,
    MouseLeftUp,
    MouseRightUp,
    #[strum(disabled)]
    #[serde(skip)]
    WorkerResponse(WorkerResponse),
    BookmarkMessage(BookmarkMessage),
    #[strum(disabled)]
    #[serde(skip)]
    PaneResize(pane_grid::ResizeEvent),
    ToggleSidebar,
    #[default]
    None,
}

impl App {
    pub fn new(
        command_tx: tokio::sync::mpsc::UnboundedSender<WorkerCommand>,
        bookmark_store: BookmarkStore,
    ) -> Self {
        let (mut ps, p) = pane_grid::State::new(Pane {
            id: 0,
            pane_type: PaneType::Pdf,
        });
        if let Some((_, split)) = ps.split(pane_grid::Axis::Vertical, p, Pane {
            id: 1,
            pane_type: PaneType::Sidebar,
        }) {
            ps.resize(split, 0.7);
        }
        Self {
            pdfs: vec![],
            pdf_idx: 0,
            file_watcher: None,
            dark_mode: false,
            invert_pdf: false,
            bookmark_store,
            command_tx,
            pane_state: ps,
            pane_ratio: 0.7,
            sidebar_showing: false,
        }
    }
    pub fn update(&mut self, message: AppMessage) -> iced::Task<AppMessage> {
        match message {
            AppMessage::OpenFile(path_buf) => {
                let path_buf = canonicalize(path_buf).unwrap();
                if self.pdfs.is_empty() {
                    self.pdfs.push(PdfViewer::new(self.command_tx.clone()));
                    self.pdf_idx = 0;
                }
                if let Some(sender) = &self.file_watcher {
                    // We should never fill this up from here
                    let _ = sender.blocking_send(WatchMessage::StartWatch(path_buf.clone()));
                }
                self.pdfs[self.pdf_idx]
                    .update(PdfMessage::OpenFile(path_buf))
                    .map(AppMessage::PdfMessage)
            }
            AppMessage::CloseFile(path_buf) => {
                let path_buf = canonicalize(path_buf).unwrap();
                if let Some(idx) = self.pdfs.iter().position(|p| p.path == path_buf) {
                    iced::Task::done(AppMessage::CloseTab(idx))
                } else {
                    iced::Task::none()
                }
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
                    self.pdfs.push(PdfViewer::new(self.command_tx.clone()));
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
            AppMessage::ToggleDarkModeUi => {
                self.dark_mode = !self.dark_mode;
                iced::Task::none()
            }
            AppMessage::ToggleDarkModePdf => {
                self.invert_pdf = !self.invert_pdf;
                for pdf in &mut self.pdfs {
                    pdf.invert_colors = self.invert_pdf;
                    pdf.force_invalidate_cache();
                }
                iced::Task::none()
            }
            AppMessage::None => iced::Task::none(),
            AppMessage::MouseMoved(vector) => {
                if !self.pdfs.is_empty() {
                    let _ = self.pdfs[self.pdf_idx].update(PdfMessage::MouseMoved(vector));
                }
                iced::Task::none()
            }
            AppMessage::MouseLeftDown => {
                if !self.pdfs.is_empty() {
                    let _ = self.pdfs[self.pdf_idx].update(PdfMessage::MouseLeftDown);
                }
                iced::Task::none()
            }
            AppMessage::MouseRightDown => {
                if !self.pdfs.is_empty() {
                    let _ = self.pdfs[self.pdf_idx].update(PdfMessage::MouseRightDown);
                }
                iced::Task::none()
            }
            AppMessage::MouseLeftUp => {
                if !self.pdfs.is_empty() {
                    let _ = self.pdfs[self.pdf_idx].update(PdfMessage::MouseLeftUp);
                }
                iced::Task::none()
            }
            AppMessage::MouseRightUp => {
                if !self.pdfs.is_empty() {
                    let _ = self.pdfs[self.pdf_idx].update(PdfMessage::MouseRightUp);
                }
                iced::Task::none()
            }
            AppMessage::WorkerResponse(worker_response) => {
                if !self.pdfs.is_empty() {
                    self.pdfs[self.pdf_idx]
                        .update(PdfMessage::WorkerResponse(worker_response))
                        .map(AppMessage::PdfMessage)
                } else {
                    iced::Task::none()
                }
            }
            AppMessage::BookmarkMessage(BookmarkMessage::RequestNewBookmark { name }) => {
                let path = self.pdfs.get(self.pdf_idx).map(|pdf| pdf.path.clone());
                let page = self.pdfs.get(self.pdf_idx).map(|pdf| pdf.cur_page_idx);
                if let (Some(path), Some(page)) = (path, page) {
                    self.bookmark_store
                        .update(BookmarkMessage::CreateBookmark {
                            path,
                            name,
                            page: (page + 1) as usize,
                        })
                        .map(AppMessage::BookmarkMessage)
                } else {
                    iced::Task::none()
                }
            }
            AppMessage::BookmarkMessage(bookmark_message) => self
                .bookmark_store
                .update(bookmark_message)
                .map(AppMessage::BookmarkMessage),
            AppMessage::PaneResize(pane_grid::ResizeEvent { split, ratio }) => {
                self.pane_state.resize(split, ratio);
                iced::Task::none()
            }
            AppMessage::ToggleSidebar => {
                self.sidebar_showing = !self.sidebar_showing;
                iced::Task::none()
            }
        }
    }

    pub fn view(&self) -> iced::Element<'_, AppMessage> {
        let menu_tpl_1 = |items| Menu::new(items).max_width(180.0).offset(0.0).spacing(0.0);
        let cfg = CONFIG.read().unwrap();

        #[rustfmt::skip]
        let mb = container(
            menu_bar!((
                debug_button_s("File"),
                menu_tpl_1(menu_items!((menu_button(
                    "Open",
                    AppMessage::OpenNewFileFinder,
                    None,
                ))(menu_button(
                    "Close",
                    AppMessage::CloseTab(self.pdf_idx),
                    None,
                ))))
            )(
                debug_button_s("View"),
                menu_tpl_1(menu_items!(
                    (menu_button(
                        if self.dark_mode {
                            "Light Interface"
                        } else {
                            "Dark Interface"
                        },
                        AppMessage::ToggleDarkModeUi,
                        None
                    ))
                    (menu_button(
                        if self.invert_pdf {
                            "Light Pdf"
                        } else {
                            "Dark Pdf"
                        },
                        AppMessage::ToggleDarkModePdf,
                        cfg.get_binding_for_msg(BindableMessage::ToggleDarkModePdf)
                    ))
                    (menu_button(
                        "Zoom In",
                        AppMessage::PdfMessage(PdfMessage::ZoomIn),
                        cfg.get_binding_for_msg(BindableMessage::ZoomIn)
                    ))
                    (menu_button(
                        "Zoom Out",
                        AppMessage::PdfMessage(PdfMessage::ZoomOut),
                        cfg.get_binding_for_msg(BindableMessage::ZoomOut)
                    ))
                    (menu_button(
                        "Zoom 100%",
                        AppMessage::PdfMessage(PdfMessage::ZoomHome),
                        cfg.get_binding_for_msg(BindableMessage::ZoomHome)
                    ))
                    (menu_button(
                        "Fit To Screen",
                        AppMessage::PdfMessage(PdfMessage::ZoomFit),
                        cfg.get_binding_for_msg(BindableMessage::ZoomFit)
                    ))
                    (menu_button(
                        if self.sidebar_showing { "Close sidebar" } else { "Open sidebar" },
                        AppMessage::ToggleSidebar,
                        cfg.get_binding_for_msg(BindableMessage::ToggleSidebar)
                    ))
                ))
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
            self.pdfs[self.pdf_idx].view().map(AppMessage::PdfMessage)
        } else {
            vertical_space().into()
        };

        let mut command_bar = widget::Row::new();
        for (i, pdf) in self.pdfs.iter().enumerate() {
            command_bar = command_bar.push(file_tab(
                &pdf.label,
                AppMessage::OpenTab(i),
                AppMessage::CloseTab(i),
                i == self.pdf_idx,
            ));
        }
        command_bar = command_bar.spacing(4.0).height(Length::Shrink);
        let tabs = scrollable(command_bar).direction(Direction::Horizontal(
            Scrollbar::default().scroller_width(0.0).width(0.0),
        ));

        let c = if self.sidebar_showing {
            let pg = PaneGrid::new(&self.pane_state, |id, pane, is_maximized| {
                pane_grid::Content::new(responsive(move |size| match pane.pane_type {
                    PaneType::Sidebar => {
                        self.bookmark_store.view().map(AppMessage::BookmarkMessage)
                    }
                    PaneType::Pdf => {
                        if !self.pdfs.is_empty() {
                            self.pdfs[self.pdf_idx].view().map(AppMessage::PdfMessage)
                        } else {
                            vertical_space().into()
                        }
                    }
                }))
                .style(|theme: &Theme| container::Style {
                    ..Default::default()
                })
            })
            .on_resize(10, AppMessage::PaneResize);

            widget::column![mb, pg, tabs]
        } else {
            widget::column![mb, self.view_pdf(), tabs]
        };

        c.into()
    }

    fn view_pdf(&self) -> Element<'_, AppMessage> {
        if !self.pdfs.is_empty() {
            self.pdfs[self.pdf_idx].view().map(AppMessage::PdfMessage)
        } else {
            vertical_space().into()
        }
    }

    pub fn subscription(&self) -> Subscription<AppMessage> {
        let keys = listen_with(|event, status, _| match event {
            Event::Mouse(e) => match e {
                iced::mouse::Event::CursorMoved { position } => {
                    Some(AppMessage::MouseMoved(position.into()))
                }
                iced::mouse::Event::ButtonPressed(button) => match button {
                    iced::mouse::Button::Left => Some(AppMessage::MouseLeftDown),
                    iced::mouse::Button::Right => Some(AppMessage::MouseRightUp),
                    iced::mouse::Button::Middle => None,
                    iced::mouse::Button::Back => {
                        Some(AppMessage::PdfMessage(PdfMessage::PreviousPage))
                    }
                    iced::mouse::Button::Forward => {
                        Some(AppMessage::PdfMessage(PdfMessage::NextPage))
                    }
                    iced::mouse::Button::Other(_) => None,
                },
                iced::mouse::Event::ButtonReleased(button) => match button {
                    iced::mouse::Button::Left => Some(AppMessage::MouseLeftUp),
                    iced::mouse::Button::Right => Some(AppMessage::MouseRightUp),
                    iced::mouse::Button::Middle => None,
                    _ => None,
                },
                iced::mouse::Event::WheelScrolled { delta } => match delta {
                    iced::mouse::ScrollDelta::Lines { x: _, y } => {
                        if y > 0.0 {
                            Some(AppMessage::PdfMessage(PdfMessage::ZoomIn))
                        } else if y < 0.0 {
                            Some(AppMessage::PdfMessage(PdfMessage::ZoomOut))
                        } else {
                            None
                        }
                    }
                    iced::mouse::ScrollDelta::Pixels { x: _, y } => {
                        if y > 0.0 {
                            Some(AppMessage::PdfMessage(PdfMessage::ZoomIn))
                        } else if y < 0.0 {
                            Some(AppMessage::PdfMessage(PdfMessage::ZoomOut))
                        } else {
                            None
                        }
                    }
                },
                _ => None,
            },
            Event::Keyboard(e) => {
                let mut config = CONFIG.write().unwrap();
                match status {
                    iced::event::Status::Ignored => {
                        config.keyboard.dispatch(e).map(|x| (*x).into())
                    }
                    iced::event::Status::Captured => None,
                }
            }
            _ => None,
        });

        let mut subs = vec![
            keys,
            Subscription::run(file_watcher).map(AppMessage::FileWatcher),
            Subscription::run(worker_responder).map(AppMessage::WorkerResponse),
        ];

        let config = CONFIG.read().unwrap();
        if config.rpc_enabled {
            subs.push(Subscription::run(rpc_server));
        }

        Subscription::batch(subs)
    }
}

impl Drop for App {
    fn drop(&mut self) {
        self.bookmark_store.save().unwrap()
    }
}

fn worker_responder() -> impl Stream<Item = WorkerResponse> {
    stream::channel(100, |mut output| async move {
        let wrx = WORKER_RX.get().unwrap();
        loop {
            let msg = {
                let mut worker_rx = wrx.lock().await;
                worker_rx.recv().await
            };
            match msg {
                Some(msg) => {
                    output.send(msg).await.unwrap();
                }
                None => break, // Channel closed
            }
        }
    })
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

#[allow(dead_code)]
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

fn format_key_sequence(seq: &KeySeq) -> String {
    let parts = seq.as_slice().iter().map(|inp| format!("{} ", inp));
    let mut out = String::from("(");
    for p in parts {
        out.push_str(&p);
    }
    out.pop();
    out.push_str(")");
    out
}

fn menu_button(
    label: &str,
    msg: AppMessage,
    binding: Option<Keybind<BindableMessage>>,
) -> button::Button<AppMessage, iced::Theme, iced::Renderer> {
    let txt = format!(
        " {}",
        binding.map_or(String::new(), |b| format_key_sequence(&b.seq))
    );
    base_button(
        row![
            text(label),
            text(txt).style(|theme: &Theme| {
                let palette = theme.extended_palette();
                text::Style {
                    color: Some(palette.primary.base.color),
                }
            })
        ],
        msg,
    )
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
