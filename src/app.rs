use std::{fs::canonicalize, path::PathBuf};

use iced::{
    alignment,
    border::{self, Radius},
    event::listen_with,
    exit,
    font::{Font, Weight},
    theme::palette,
    widget::{
        self,
        button,
        container,
        horizontal_space,
        pane_grid,
        responsive,
        row,
        scrollable,
        scrollable::{Direction, Scrollbar}, // -
        stack,
        text,
        vertical_space,
        PaneGrid,
    },
    Background, Border, Element, Event, Length, Padding, Shadow, Subscription, Theme,
};
use iced_aw::{Menu, iced_fonts::REQUIRED_FONT, menu::primary, menu_items};
use iced_aw::{
    menu::{self, Item},
    menu_bar,
};
use iced_fonts::required::{RequiredIcons, icon_to_string};
use keybinds::{KeySeq, Keybind};
use rfd::AsyncFileDialog;
use serde::{Deserialize, Serialize};
use strum::EnumString;
use tokio::sync::mpsc;
use tracing::{debug, error};

use crate::{
    bookmarks::{BookmarkMessage, BookmarkStore},
    config::{BindableMessage, MouseAction, MouseButton, MouseInput, MouseModifiers},
    geometry::Vector,
    icons,
    pdf::{PdfMessage, widget::PdfViewer, outline_extraction::OutlineItem},
    rpc::rpc_server,
    watch::{WatchMessage, WatchNotification, file_watcher},
    CONFIG,
};

#[derive(Debug)]
enum PaneType {
    Pdf,
    Sidebar,
}

#[derive(Debug)]
struct Pane {
    pane_type: PaneType,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, EnumString, Default)]
pub enum SidebarTab {
    #[default]
    Outline,
    Bookmark,
}

#[derive(Debug)]
pub struct App {
    pub pdfs: Vec<PdfViewer>,
    pub pdf_idx: usize,
    pub file_watcher: Option<mpsc::Sender<WatchMessage>>,
    pub dark_mode: bool,
    pub invert_pdf: bool,
    bookmark_store: BookmarkStore,
    pane_state: pane_grid::State<Pane>,
    sidebar_tab: SidebarTab,
    waiting_for_worker: Vec<AppMessage>,
    shift_pressed: bool,
    ctrl_pressed: bool,
    scale_factor: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, EnumString, Default)]
pub enum AppMessage {
    OpenFile(PathBuf),
    CloseFile(PathBuf),
    OpenNewFileFinder,
    #[strum(disabled)]
    #[serde(skip)]
    FileDialogResult(Option<PathBuf>),
    Debug(String),
    PdfMessage(PdfMessage),
    OpenTab(usize),
    CloseTab(usize),
    CloseActiveTab,
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
    MouseMiddleDown,
    MouseBackDown,
    MouseForwardDown,
    MouseLeftUp,
    MouseRightUp,
    MouseMiddleUp,
    MouseBackUp,
    MouseForwardUp,
    ShiftPressed(bool),
    CtrlPressed(bool),
    BookmarkMessage(BookmarkMessage),
    #[strum(disabled)]
    #[serde(skip)]
    PaneResize(pane_grid::ResizeEvent),
    ToggleSidebar,
    SetSidebar(SidebarTab),
    OutlineGoToPage(u32),
    #[default]
    None,
}

impl App {
    fn get_mouse_action(&self, button: MouseButton) -> Option<MouseAction> {
        let input = MouseInput {
            button,
            modifiers: MouseModifiers {
                ctrl: self.ctrl_pressed,
                shift: self.shift_pressed,
            },
        };
        CONFIG.read().unwrap().get_mouse_action(input)
    }

    pub fn new(bookmark_store: BookmarkStore) -> Self {
        let (ps, _p) = pane_grid::State::new(Pane {
            pane_type: PaneType::Pdf,
        });
        Self {
            pdfs: vec![],
            pdf_idx: 0,
            file_watcher: None,
            dark_mode: true,
            invert_pdf: false,
            bookmark_store,
            pane_state: ps,
            sidebar_tab: SidebarTab::Outline,
            waiting_for_worker: vec![],
            shift_pressed: false,
            ctrl_pressed: false,
            scale_factor: CONFIG.read().unwrap().scale_factor,
        }
    }

    fn has_sidebar_pane(&self) -> bool {
        self.pane_state
            .panes
            .iter()
            .any(|(_, pane)| matches!(pane.pane_type, PaneType::Sidebar))
    }

    fn get_pdf_pane_id(&self) -> Option<pane_grid::Pane> {
        self.pane_state
            .panes
            .iter()
            .find(|(_, pane)| matches!(pane.pane_type, PaneType::Pdf))
            .map(|(id, _)| *id)
    }

    fn get_sidebar_pane_id(&self) -> Option<pane_grid::Pane> {
        self.pane_state
            .panes
            .iter()
            .find(|(_, pane)| matches!(pane.pane_type, PaneType::Sidebar))
            .map(|(id, _)| *id)
    }

    pub fn update(&mut self, message: AppMessage) -> iced::Task<AppMessage> {
        match message {
            AppMessage::OpenFile(path_buf) => {
                let path_buf = canonicalize(path_buf).unwrap();
                let out = match PdfViewer::from_path(path_buf.clone()) {
                    Ok(mut viewer) => {
                        viewer.set_scale_factor(self.scale_factor);
                        self.pdfs.push(viewer);
                        iced::Task::done(AppMessage::OpenTab(self.pdfs.len() - 1))
                    }
                    Err(e) => {
                        error!("Couldn't create pdf viewer or {path_buf:?} {e}");
                        iced::Task::none()
                    }
                };
                if let Some(sender) = self.file_watcher.as_ref() {
                    // We should never fill this up from here, thus blocking is allright
                    let _ = sender.blocking_send(WatchMessage::StartWatch(path_buf.clone()));
                }
                out
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
            AppMessage::OpenNewFileFinder => iced::Task::perform(
                async {
                    AsyncFileDialog::new()
                        .add_filter("Pdf", &["pdf"])
                        .pick_file()
                        .await
                        .map(|file_handle| file_handle.path().to_path_buf())
                },
                AppMessage::FileDialogResult,
            ),
            AppMessage::FileDialogResult(path_buf_opt) => path_buf_opt
                .map_or(iced::Task::none(), |path_buf| {
                    iced::Task::done(AppMessage::OpenFile(path_buf))
                }),
            AppMessage::CloseTab(i) => {
                if self.pdfs.is_empty() {
                    exit()
                } else {
                    if let Some(sender) = &self.file_watcher {
                        // We should never fill this up from here
                        let _ = sender.blocking_send(WatchMessage::StopWatch(
                            self.pdfs[self.pdf_idx].path.clone(),
                        ));
                    }
                    self.pdfs.remove(i);
                    if self.pdf_idx >= self.pdfs.len() {
                        if self.pdfs.is_empty() {
                            self.pdf_idx = 0;
                        } else {
                            self.pdf_idx = self.pdfs.len() - 1;
                        }
                    }
                    iced::Task::none()
                }
            }
            AppMessage::PreviousTab => {
                self.pdf_idx = if self.pdf_idx == 0 {
                    0
                } else {
                    self.pdf_idx - 1
                };
                iced::Task::none()
            }
            AppMessage::NextTab => {
                self.pdf_idx = (self.pdf_idx + 1).min(self.pdfs.len() - 1);
                iced::Task::none()
            }
            AppMessage::OpenTab(i) => {
                self.pdf_idx = i;
                iced::Task::none()
            }
            AppMessage::FileWatcher(watch_notification) => {
                match watch_notification {
                    WatchNotification::Ready(sender) => {
                        self.file_watcher = Some(sender);
                    }
                    WatchNotification::Changed(path) => {
                        self.pdfs
                            .iter_mut()
                            .find(|pdf| pdf.path == path)
                            .map(|viewer| viewer.update(PdfMessage::FileChanged));
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
                    let _ = self.pdfs[self.pdf_idx]
                        .update(PdfMessage::MouseLeftDown(self.shift_pressed));
                }
                iced::Task::none()
            }
            AppMessage::MouseRightDown => {
                if !self.pdfs.is_empty()
                    && let Some(action) = self.get_mouse_action(MouseButton::Right)
                {
                    let _ = self.pdfs[self.pdf_idx].update(PdfMessage::MouseAction(action, true));
                }
                iced::Task::none()
            }
            AppMessage::MouseMiddleDown => {
                if !self.pdfs.is_empty()
                    && let Some(action) = self.get_mouse_action(MouseButton::Middle)
                {
                    let _ = self.pdfs[self.pdf_idx].update(PdfMessage::MouseAction(action, true));
                }
                iced::Task::none()
            }
            AppMessage::MouseLeftUp => {
                if !self.pdfs.is_empty() {
                    let _ =
                        self.pdfs[self.pdf_idx].update(PdfMessage::MouseLeftUp(self.shift_pressed));
                }
                iced::Task::none()
            }
            AppMessage::MouseRightUp => {
                if !self.pdfs.is_empty()
                    && let Some(action) = self.get_mouse_action(MouseButton::Right)
                {
                    let _ = self.pdfs[self.pdf_idx].update(PdfMessage::MouseAction(action, false));
                }
                iced::Task::none()
            }
            AppMessage::MouseMiddleUp => {
                if !self.pdfs.is_empty()
                    && let Some(action) = self.get_mouse_action(MouseButton::Middle)
                {
                    let _ = self.pdfs[self.pdf_idx].update(PdfMessage::MouseAction(action, false));
                }
                iced::Task::none()
            }
            AppMessage::MouseBackDown => {
                if !self.pdfs.is_empty()
                    && let Some(action) = self.get_mouse_action(MouseButton::Back)
                {
                    let _ = self.pdfs[self.pdf_idx].update(PdfMessage::MouseAction(action, true));
                }
                iced::Task::none()
            }
            AppMessage::MouseBackUp => {
                if !self.pdfs.is_empty()
                    && let Some(action) = self.get_mouse_action(MouseButton::Back)
                {
                    let _ = self.pdfs[self.pdf_idx].update(PdfMessage::MouseAction(action, false));
                }
                iced::Task::none()
            }
            AppMessage::MouseForwardDown => {
                if !self.pdfs.is_empty()
                    && let Some(action) = self.get_mouse_action(MouseButton::Forward)
                {
                    let _ = self.pdfs[self.pdf_idx].update(PdfMessage::MouseAction(action, true));
                }
                iced::Task::none()
            }
            AppMessage::MouseForwardUp => {
                if !self.pdfs.is_empty()
                    && let Some(action) = self.get_mouse_action(MouseButton::Forward)
                {
                    let _ = self.pdfs[self.pdf_idx].update(PdfMessage::MouseAction(action, false));
                }
                iced::Task::none()
            }
            AppMessage::ShiftPressed(pressed) => {
                self.shift_pressed = pressed;
                iced::Task::none()
            }
            AppMessage::CtrlPressed(pressed) => {
                self.ctrl_pressed = pressed;
                iced::Task::none()
            }
            AppMessage::BookmarkMessage(BookmarkMessage::RequestNewBookmark { name }) => {
                let path = self.pdfs.get(self.pdf_idx).map(|pdf| pdf.path.clone());
                let page = self.pdfs.get(self.pdf_idx).map(|pdf| pdf.cur_page_idx);
                if let (Some(path), Some(page)) = (path, page) {
                    self.bookmark_store
                        .update(BookmarkMessage::CreateBookmark { path, name, page })
                        .map(AppMessage::BookmarkMessage)
                } else {
                    iced::Task::none()
                }
            }
            AppMessage::BookmarkMessage(BookmarkMessage::GoTo { path, page }) => {
                match self
                    .pdfs
                    .iter_mut()
                    .enumerate()
                    .find(|(_, pdf)| pdf.path == path)
                {
                    Some((i, pdf)) => iced::Task::done(AppMessage::OpenTab(i)).chain(
                        pdf.update(PdfMessage::SetPage(page))
                            .map(AppMessage::PdfMessage),
                    ),
                    None => {
                        self.waiting_for_worker.push(AppMessage::BookmarkMessage(
                            BookmarkMessage::GoTo {
                                path: path.clone(),
                                page,
                            },
                        ));
                        iced::Task::done(AppMessage::OpenFile(path))
                    }
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
                if self.has_sidebar_pane() {
                    if let Some(sidebar_id) = self.get_sidebar_pane_id() {
                        self.pane_state.close(sidebar_id);
                    }
                } else {
                    if let Some(pdf_id) = self.get_pdf_pane_id() {
                        if let Some((_, split)) = self.pane_state.split(
                            pane_grid::Axis::Vertical,
                            pdf_id,
                            Pane {
                                pane_type: PaneType::Sidebar,
                            },
                        ) {
                            self.pane_state.resize(split, 0.7);
                        }
                    }
                }
                iced::Task::none()
            }
            AppMessage::SetSidebar(sidebar_tab) => {
                self.sidebar_tab = sidebar_tab;
                iced::Task::none()
            }
            AppMessage::OutlineGoToPage(page) => {
                if !self.pdfs.is_empty() {
                    self.pdfs[self.pdf_idx]
                        .update(PdfMessage::SetPage(page as i32))
                        .map(AppMessage::PdfMessage)
                } else {
                    iced::Task::none()
                }
            }
            AppMessage::CloseActiveTab => iced::Task::done(AppMessage::CloseTab(self.pdf_idx)),
        }
    }

    fn create_menu_bar(&self) -> Element<'_, AppMessage> {
        let menu_tpl_1 = |items| Menu::new(items).max_width(180.0).offset(0.0).spacing(0.0);
        let cfg = CONFIG.read().unwrap();

        let exit_close_label = if self.pdfs.is_empty() {
            "Exit"
        } else {
            "Close"
        };

        container(row![
            menu_bar!((
                debug_button_s("File"),
                menu_tpl_1(menu_items!((menu_button(
                    "Open",
                    AppMessage::OpenNewFileFinder,
                    cfg.get_binding_for_msg(BindableMessage::OpenFileFinder)
                ))(menu_button(
                    "Print",
                    AppMessage::PdfMessage(PdfMessage::PrintPdf),
                    cfg.get_binding_for_msg(BindableMessage::PrintPdf)
                ))(menu_button_last(
                    exit_close_label,
                    AppMessage::CloseTab(self.pdf_idx),
                    None,
                ))))
            )(
                debug_button_s("View"),
                menu_tpl_1(menu_items!((menu_button(
                    if self.dark_mode {
                        "Light Interface"
                    } else {
                        "Dark Interface"
                    },
                    AppMessage::ToggleDarkModeUi,
                    cfg.get_binding_for_msg(BindableMessage::ToggleDarkModeUi)
                ))(menu_button(
                    if self.invert_pdf {
                        "Light Pdf"
                    } else {
                        "Dark Pdf"
                    },
                    AppMessage::ToggleDarkModePdf,
                    cfg.get_binding_for_msg(BindableMessage::ToggleDarkModePdf)
                ))(menu_button(
                    "Zoom In",
                    AppMessage::PdfMessage(PdfMessage::ZoomIn),
                    cfg.get_binding_for_msg(BindableMessage::ZoomIn)
                ))(menu_button(
                    "Zoom Out",
                    AppMessage::PdfMessage(PdfMessage::ZoomOut),
                    cfg.get_binding_for_msg(BindableMessage::ZoomOut)
                ))(menu_button(
                    "Zoom 100%",
                    AppMessage::PdfMessage(PdfMessage::ZoomHome),
                    cfg.get_binding_for_msg(BindableMessage::ZoomHome)
                ))(menu_button(
                    "Fit To Screen",
                    AppMessage::PdfMessage(PdfMessage::ZoomFit),
                    cfg.get_binding_for_msg(BindableMessage::ZoomFit)
                ))(menu_button_last(
                    if self.has_sidebar_pane() {
                        "Close sidebar"
                    } else {
                        "Open sidebar"
                    },
                    AppMessage::ToggleSidebar,
                    cfg.get_binding_for_msg(BindableMessage::ToggleSidebar)
                ))))
            ))
            .draw_path(menu::DrawPath::Backdrop)
            .style(
                |theme: &iced::Theme, status: iced_aw::style::Status| menu::Style {
                    menu_background: theme.extended_palette().background.weak.color.into(),
                    menu_background_expand: Padding::default().bottom(3.0).left(3.0).right(3.0),
                    bar_background_expand: 0.0.into(),
                    bar_background: theme.extended_palette().background.weak.color.into(),
                    menu_border: Border {
                        radius: Radius::new(0.0).bottom(8.0),
                        color: theme.extended_palette().background.strong.color.into(),
                        width: 2.0,
                    },
                    bar_shadow: Shadow::default(),
                    menu_shadow: Shadow::default(),
                    ..primary(theme, status)
                },
            ),
            container(horizontal_space())
                .width(Length::Fill)
                .height(28.0)
                .style(|theme: &iced::Theme| container::Style {
                    background: Some(theme.extended_palette().background.weak.color.into()),
                    ..Default::default()
                })
        ])
        .width(Length::Fill)
        .padding(Padding::default().bottom(2.0))
        .style(|theme| container::Style {
            background: Some(Background::Color(
                theme.extended_palette().background.weak.color,
            )),
            border: Border {
                color: theme.extended_palette().background.strong.color.into(),
                width: 2.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        })
        .into()
    }

    fn create_tabs(&self) -> Element<'_, AppMessage> {
        let mut command_bar = widget::Row::new();
        for (i, pdf) in self.pdfs.iter().enumerate() {
            command_bar = command_bar.push(file_tab(
                &pdf.name,
                &pdf.page_progress,
                AppMessage::OpenTab(i),
                AppMessage::CloseTab(i),
                i == self.pdf_idx,
            ));
        }
        command_bar = command_bar.spacing(4.0).height(Length::Shrink);
        scrollable(command_bar)
            .direction(Direction::Horizontal(
                Scrollbar::default().scroller_width(0.0).width(0.0),
            ))
            .into()
    }

    pub fn view(&self) -> iced::Element<'_, AppMessage> {
        let pg = PaneGrid::new(&self.pane_state, |_id, pane, _is_maximized| {
            pane_grid::Content::new(match pane.pane_type {
                PaneType::Sidebar => self.view_sidebar(),
                PaneType::Pdf => {
                    let menu_bar = self.create_menu_bar();
                    let pdf_content = if self.pdfs.is_empty() {
                        vertical_space().into()
                    } else {
                        self.pdfs[self.pdf_idx].view().map(AppMessage::PdfMessage)
                    };
                    let tabs = self.create_tabs();

                    widget::column![
                        menu_bar,
                        stack![
                            pdf_content,
                            container(tabs)
                                .align_y(alignment::Vertical::Bottom)
                                .width(Length::Fill)
                                .height(Length::Fill)
                                .padding(8.0)
                        ]
                    ]
                    .into()
                }
            })
            .style(|_theme: &Theme| Default::default())
        })
        .on_resize(10, AppMessage::PaneResize);

        pg.into()
    }

    fn view_sidebar(&self) -> Element<'_, AppMessage> {
        let create_expanded_outline_button = || {
            button(
                widget::row![
                    widget::svg(icons::table_of_contents())
                        .width(18.0)
                        .height(18.0)
                        .style(|theme: &Theme, _| {
                            let palette = theme.extended_palette();
                            widget::svg::Style {
                                color: Some(palette.primary.base.text),
                            }
                        }),
                    horizontal_space().width(8.0),
                    text("Outline")
                ]
                .align_y(alignment::Vertical::Center),
            )
            .width(Length::Fill)
            .height(30.0)
            .padding(6.0)
            .style(|theme: &Theme, _status| {
                let palette = theme.extended_palette();
                button::Style {
                    background: Some(palette.primary.base.color.into()),
                    text_color: palette.primary.base.text,
                    border: Border {
                        radius: Radius::from(4.0),
                        ..Default::default()
                    },
                    ..Default::default()
                }
            })
            .on_press(AppMessage::SetSidebar(SidebarTab::Outline))
        };

        let create_collapsed_outline_button = || {
            button(
                widget::svg(icons::table_of_contents())
                    .width(18.0)
                    .height(18.0)
                    .style(|theme: &Theme, _| {
                        let palette = theme.extended_palette();
                        widget::svg::Style {
                            color: Some(palette.primary.base.text),
                        }
                    }),
            )
            .width(Length::Shrink)
            .height(30.0)
            .padding(6.0)
            .style(|theme: &Theme, status| {
                let palette = theme.extended_palette();
                widget::button::Style {
                    background: match status {
                        widget::button::Status::Hovered => Some(palette.primary.weak.color.into()),
                        widget::button::Status::Pressed => {
                            Some(palette.primary.strong.color.into())
                        }
                        widget::button::Status::Active => Some(palette.primary.base.color.into()),
                        _ => None,
                    },
                    border: Border {
                        radius: 4.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }
            })
            .on_press(AppMessage::SetSidebar(SidebarTab::Outline))
        };

        let create_expanded_bookmark_button = || {
            button(
                widget::row![
                    text("Bookmarks").width(Length::Fill),
                    widget::svg(icons::bookmark())
                        .width(18.0)
                        .height(18.0)
                        .style(|theme: &Theme, _| {
                            let palette = theme.extended_palette();
                            widget::svg::Style {
                                color: Some(palette.primary.base.text),
                            }
                        })
                ]
                .align_y(alignment::Vertical::Center),
            )
            .width(Length::Fill)
            .height(30.0)
            .padding(6.0)
            .style(|theme: &Theme, _status| {
                let palette = theme.extended_palette();
                button::Style {
                    background: Some(palette.primary.base.color.into()),
                    text_color: palette.primary.base.text,
                    border: Border {
                        radius: Radius::from(4.0),
                        ..Default::default()
                    },
                    ..Default::default()
                }
            })
            .on_press(AppMessage::SetSidebar(SidebarTab::Bookmark))
        };

        let create_collapsed_bookmark_button = || {
            button(
                widget::svg(icons::bookmark())
                    .width(18.0)
                    .height(18.0)
                    .style(|theme: &Theme, _| {
                        let palette = theme.extended_palette();
                        widget::svg::Style {
                            color: Some(palette.primary.base.text),
                        }
                    }),
            )
            .width(Length::Shrink)
            .height(30.0)
            .padding(6.0)
            .style(|theme: &Theme, status| {
                let palette = theme.extended_palette();
                widget::button::Style {
                    background: match status {
                        widget::button::Status::Hovered => Some(palette.primary.weak.color.into()),
                        widget::button::Status::Pressed => {
                            Some(palette.primary.strong.color.into())
                        }
                        widget::button::Status::Active => Some(palette.primary.base.color.into()),
                        _ => None,
                    },
                    border: Border {
                        radius: 4.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }
            })
            .on_press(AppMessage::SetSidebar(SidebarTab::Bookmark))
        };

        let (outline_button, bookmark_button) = match self.sidebar_tab {
            SidebarTab::Outline => (
                create_expanded_outline_button(),
                create_collapsed_bookmark_button(),
            ),
            SidebarTab::Bookmark => (
                create_collapsed_outline_button(),
                create_expanded_bookmark_button(),
            ),
        };

        let sidebar_picker = widget::row![outline_button, bookmark_button]
            .height(Length::Shrink)
            .spacing(4.0)
            .padding(Padding::default().top(4.0).bottom(4.0));

        let contents: Element<'_, AppMessage> = match self.sidebar_tab {
            SidebarTab::Outline => self.view_outline(),
            SidebarTab::Bookmark => self.bookmark_store.view().map(AppMessage::BookmarkMessage),
        };

        widget::column![
            sidebar_picker,
            widget::vertical_space().height(8.0),
            contents,
        ]
        .padding(8.0)
        .into()
    }

    fn view_outline(&self) -> Element<'_, AppMessage> {
        let mut col = widget::column![
            text("Document Outline").size(18.0),
            vertical_space().height(8.0),
        ];

        if self.pdfs.is_empty() {
            col = col.push(text("No document loaded").style(|theme: &Theme| {
                let palette = theme.extended_palette();
                text::Style {
                    color: Some(palette.background.weak.color),
                }
            }));
        } else {
            let outline = self.pdfs[self.pdf_idx].get_outline();
            if outline.is_empty() {
                col = col.push(text("No outline available").style(|theme: &Theme| {
                    let palette = theme.extended_palette();
                    text::Style {
                        color: Some(palette.background.weak.color),
                    }
                }));
            } else {
                let outline_content = view_outline_items(outline, 0);
                col = col.push(widget::scrollable(outline_content));
            }
        }
        container(col).height(Length::Fill).into()
    }

    pub fn subscription(&self) -> Subscription<AppMessage> {
        let keys = listen_with(|event, status, _| match event {
            Event::Keyboard(keyboard_event) => match keyboard_event {
                iced::keyboard::Event::ModifiersChanged(modifiers) => {
                    // We need to handle both modifiers, but we can only return one message
                    // We'll prioritize shift for now and handle ctrl separately
                    // TODO: This is a limitation - we should handle both modifiers properly
                    Some(AppMessage::ShiftPressed(modifiers.shift()))
                }
                _ => {
                    // Handle other keyboard events for keybinds
                    let mut config = CONFIG.write().unwrap();
                    match status {
                        iced::event::Status::Ignored => config
                            .keyboard
                            .dispatch(keyboard_event)
                            .map(|x| (*x).into()),
                        iced::event::Status::Captured => None,
                    }
                }
            },
            Event::Mouse(e) => match e {
                iced::mouse::Event::CursorMoved { position } => {
                    Some(AppMessage::MouseMoved(position.into()))
                }
                iced::mouse::Event::ButtonPressed(button) => match button {
                    iced::mouse::Button::Left => Some(AppMessage::MouseLeftDown),
                    iced::mouse::Button::Right => Some(AppMessage::MouseRightDown),
                    iced::mouse::Button::Middle => Some(AppMessage::MouseMiddleDown),
                    iced::mouse::Button::Back => Some(AppMessage::MouseBackDown),
                    iced::mouse::Button::Forward => Some(AppMessage::MouseForwardDown),
                    iced::mouse::Button::Other(_) => None,
                },
                iced::mouse::Event::ButtonReleased(button) => match button {
                    iced::mouse::Button::Left => Some(AppMessage::MouseLeftUp),
                    iced::mouse::Button::Right => Some(AppMessage::MouseRightUp),
                    iced::mouse::Button::Middle => Some(AppMessage::MouseMiddleUp),
                    iced::mouse::Button::Back => Some(AppMessage::MouseBackUp),
                    iced::mouse::Button::Forward => Some(AppMessage::MouseForwardUp),
                    _ => None,
                },
                iced::mouse::Event::WheelScrolled { delta } => match status {
                    iced::event::Status::Ignored => match delta {
                        iced::mouse::ScrollDelta::Lines { x: _, y }
                        | iced::mouse::ScrollDelta::Pixels { x: _, y } => {
                            if y > 0.0 {
                                Some(AppMessage::PdfMessage(PdfMessage::ZoomIn))
                            } else if y < 0.0 {
                                Some(AppMessage::PdfMessage(PdfMessage::ZoomOut))
                            } else {
                                None
                            }
                        }
                    },
                    iced::event::Status::Captured => None,
                },
                _ => None,
            },

            _ => None,
        });

        let mut subs = vec![
            keys,
            Subscription::run(file_watcher).map(AppMessage::FileWatcher),
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
        self.bookmark_store.save().unwrap();
    }
}

fn view_outline_items<'a>(items: &'a [OutlineItem], level: u32) -> widget::Column<'a, AppMessage> {
    let mut col = widget::column![];

    for item in items {
        let indent = level * 16; // 16 pixels per level

        let item_button = if let Some(page) = item.page {
            button(
                container(text(&item.title).style(|theme: &Theme| {
                    let palette = theme.extended_palette();
                    text::Style {
                        color: Some(palette.primary.base.color),
                    }
                }))
                .padding(Padding::default().left(indent as f32)),
            )
            .style(|_: &Theme, _| widget::button::Style {
                background: None,
                ..Default::default()
            })
            .width(Length::Fill)
            .on_press(AppMessage::OutlineGoToPage(page))
        } else {
            button(
                container(text(&item.title).style(|theme: &Theme| {
                    let palette = theme.extended_palette();
                    text::Style {
                        color: Some(palette.background.weak.color),
                    }
                }))
                .padding(Padding::default().left(indent as f32)),
            )
            .style(|_: &Theme, _| widget::button::Style {
                background: None,
                ..Default::default()
            })
            .width(Length::Fill)
        };

        col = col.push(item_button);

        // Recursively add children
        if !item.children.is_empty() {
            let children_col = view_outline_items(&item.children, level + 1);
            col = col.push(children_col);
        }
    }

    col
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
) -> button::Button<'_, AppMessage, iced::Theme, iced::Renderer> {
    base_button(text(label).align_y(alignment::Vertical::Center), msg)
}

#[allow(dead_code)]
fn debug_button(label: &str) -> button::Button<'_, AppMessage, iced::Theme, iced::Renderer> {
    labeled_button(label, AppMessage::Debug(label.into())).width(Length::Fill)
}

fn debug_button_s(label: &str) -> button::Button<'_, AppMessage, iced::Theme, iced::Renderer> {
    labeled_button(label, AppMessage::Debug(label.into()))
        .width(Length::Shrink)
        .style(move |theme, status| {
            let palette = theme.extended_palette();
            let pair = match status {
                button::Status::Active => palette.background.weak,
                button::Status::Hovered | button::Status::Disabled => palette.background.base,
                button::Status::Pressed => palette.primary.base,
            };
            button::Style {
                text_color: pair.text,
                background: Some(Background::Color(pair.color)),
                ..Default::default()
            }
        })
}

fn format_key_sequence(seq: &KeySeq) -> String {
    let parts = seq.as_slice().iter().map(|inp| format!("{inp} "));
    let mut out = String::from("(");
    for p in parts {
        out.push_str(&p);
    }
    out.pop();
    out.push(')');
    out
}

fn menu_button(
    label: &str,
    msg: AppMessage,
    binding: Option<Keybind<BindableMessage>>,
) -> button::Button<'_, AppMessage, iced::Theme, iced::Renderer> {
    let txt = format!(
        " {}",
        binding.map_or(String::new(), |b| format_key_sequence(&b.seq))
    );
    base_button(
        row![
            text(label),
            horizontal_space(),
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
            button::Status::Active => palette.background.weak,
            button::Status::Hovered => palette.background.base,
            button::Status::Pressed => palette.background.strong,
            button::Status::Disabled => palette.secondary.weak,
        };
        button::Style {
            text_color: pair.text,
            background: Some(Background::Color(pair.color)),
            border: Border {
                radius: Radius::default(),
                ..Default::default()
            },
            ..Default::default()
        }
    })
}

fn menu_button_last(
    label: &str,
    msg: AppMessage,
    binding: Option<Keybind<BindableMessage>>,
) -> button::Button<'_, AppMessage, iced::Theme, iced::Renderer> {
    let txt = format!(
        " {}",
        binding.map_or(String::new(), |b| format_key_sequence(&b.seq))
    );
    base_button(
        row![
            text(label),
            horizontal_space(),
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
            button::Status::Active => palette.background.weak,
            button::Status::Hovered => palette.background.base,
            button::Status::Pressed => palette.background.strong,
            button::Status::Disabled => palette.secondary.weak,
        };
        button::Style {
            text_color: pair.text,
            background: Some(Background::Color(pair.color)),
            border: Border {
                radius: Radius::default().bottom(8.0),
                color: theme.extended_palette().background.strong.color.into(),
                width: 0.0,
                ..Default::default()
            },
            ..Default::default()
        }
    })
}

fn file_tab<'a>(
    file_name: &'a str,
    page_progress: &'a str,
    on_press: AppMessage,
    on_close: AppMessage,
    is_open: bool,
) -> Element<'a, AppMessage> {
    container(
        widget::row![
            base_button(
                widget::row![
                    text(file_name).font(Font {
                        family: iced::font::Family::Name("Geist"),
                        weight: Weight::Semibold,
                        ..Default::default()
                    }),
                    text(page_progress)
                ]
                .spacing(8.0),
                on_press
            )
            .style(file_tab_style),
            // TODO: Svg X
            base_button(
                text(icon_to_string(RequiredIcons::X))
                    .align_y(alignment::Vertical::Bottom)
                    .size(24.0)
                    .font(REQUIRED_FONT),
                on_close
            )
            .padding(0.0)
            .style(file_tab_style),
        ]
        .align_y(alignment::Vertical::Center)
        .spacing(2.0),
    )
    .padding(6.0)
    .style(move |theme| {
        let palette = theme.extended_palette();
        let border_pair = if is_open {
            palette.primary.base
        } else {
            palette.primary.weak
        };
        container::Style {
            text_color: None,
            background: Some(palette.background.weak.color.into()),
            border: Border {
                color: border_pair.color,
                width: 2.0,
                radius: Radius::from(8.0),
            },
            shadow: Shadow {
                color: border_pair.color,
                offset: iced::Vector { x: 0.0, y: 2.0 },
                blur_radius: 4.0,
            },
        }
    })
    .into()
}

pub fn file_tab_style(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();
    let base = styled(palette.background.strong);

    match status {
        button::Status::Active | button::Status::Pressed | button::Status::Hovered => {
            button::Style {
                background: None,
                text_color: palette.background.base.text,
                ..base
            }
        }
        button::Status::Disabled => disabled(&base),
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

fn disabled(style: &button::Style) -> button::Style {
    button::Style {
        background: style
            .background
            .map(|background| background.scale_alpha(0.5)),
        text_color: style.text_color.scale_alpha(0.5),
        ..*style
    }
}
