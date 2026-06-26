use std::{
    fs::canonicalize,
    path::{PathBuf},
};

use iced::{
    Background, Border, Element, Event, Length, Padding, Shadow, Subscription, Theme,
    advanced::graphics::core::window,
    alignment,
    border::{self, Radius},
    event::listen_with,
    exit,
    font::{Font, Weight},
    keyboard::Modifiers,
    theme::palette,
    widget::{
        self, PaneGrid, button, container, pane_grid, scrollable,
        scrollable::{Direction, Scrollbar},
        stack, text,
    },
};
use rfd::AsyncFileDialog;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use strum::EnumString;
use tokio::sync::mpsc;
use tracing::error;

use crate::{
    CONFIG,
    bookmarks::{BookmarkMessage, BookmarkStore},
    config::{BindableMessage, MouseAction, MouseButton, MouseInput, MouseModifiers},
    geometry::Vector,
    icons,
    jumplist::{JumpLocation, Jumplist},
    pdf::{
        PdfMessage, SearchMethod,
        page_layout::PageLayout,
        widget::{OutlineItem, PdfViewer},
    },
    platform_specific,
    recent_files::RecentFiles,
    rpc::rpc_server,
    watch::{WatchMessage, WatchNotification, file_watcher},
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
    mac_menu: Option<platform_specific::macos::Menu>,
    pub pdfs: Vec<PdfViewer>,
    pub pdf_idx: usize,
    pub file_watcher: Option<mpsc::Sender<WatchMessage>>,
    pub dark_mode: bool,
    pub invert_pdf: bool,
    pub draw_page_borders: bool,
    presentation_mode: bool,
    search_open: bool,
    search_hover: bool,
    bookmark_store: BookmarkStore,
    recent_files: RecentFiles,
    pane_state: pane_grid::State<Pane>,
    sidebar_tab: SidebarTab,
    shift_pressed: bool,
    ctrl_pressed: bool,
    scale_factor: f64,
    jumplist: Jumplist,
}

#[derive(Debug, Clone, Serialize, Deserialize, EnumString, Default)]
pub enum AppMessage {
    InitializeMacMenu,
    OpenFile(PathBuf),
    #[strum(disabled)]
    #[serde(skip)]
    OpenTempFile(PathBuf),
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
    TogglePageBorders,
    MouseMoved(Vector<f32>),
    #[strum(disabled)]
    #[serde(skip)]
    MouseButtonDown(MouseButton),
    #[strum(disabled)]
    #[serde(skip)]
    MouseButtonUp(MouseButton),
    ShiftPressed(bool),
    CtrlPressed(bool),
    #[strum(disabled)]
    #[serde(skip)]
    ModifiersChanged(iced::keyboard::Modifiers),
    #[strum(disabled)]
    #[serde(skip)]
    Scroll(iced::mouse::ScrollDelta),
    #[strum(disabled)]
    #[serde(skip)]
    SearchHover(bool),
    BookmarkMessage(BookmarkMessage),
    #[strum(disabled)]
    #[serde(skip)]
    PaneResize(pane_grid::ResizeEvent),
    ToggleSidebar,
    SetSidebar(SidebarTab),
    OutlineGoToPage(usize),
    Exit,
    #[default]
    None,
    #[strum(disabled)]
    #[serde(skip)]
    FoundWindowId(Option<iced::window::Id>),
    FoundScaleFactor(f32),
    JumpTo(JumpLocation),
    JumpBack,
    JumpForward,
    ToggleFullscreen,
    TogglePresentationMode,
    OpenSearch,
    CloseSearch,
    ToggleSearchMethod,
}

impl AppMessage {
    pub fn menu_id(&self) -> Option<String> {
        match self {
            AppMessage::OpenNewFileFinder => Some(String::from("OpenFileFinder")),
            AppMessage::CloseActiveTab => Some(String::from("CloseActiveTab")),
            AppMessage::ToggleDarkModeUi => Some(String::from("ToggleDarkModeUi")),
            AppMessage::ToggleDarkModePdf => Some(String::from("ToggleDarkModePdf")),
            AppMessage::TogglePageBorders => Some(String::from("TogglePageBorders")),
            AppMessage::ToggleSidebar => Some(String::from("ToggleSidebar")),
            AppMessage::TogglePresentationMode => {
                Some(String::from("TogglePresentationMode"))
            }
            AppMessage::ToggleFullscreen => Some(String::from("ToggleFullscreen")),
            AppMessage::PdfMessage(PdfMessage::PrintPdf) => {
                Some(String::from("PrintPdf"))
            }
            AppMessage::PdfMessage(PdfMessage::ZoomIn) => Some(String::from("ZoomIn")),
            AppMessage::PdfMessage(PdfMessage::ZoomOut) => {
                Some(String::from("ZoomOut"))
            }
            AppMessage::PdfMessage(PdfMessage::ZoomHome) => {
                Some(String::from("ZoomHome"))
            }
            AppMessage::PdfMessage(PdfMessage::ZoomFit) => {
                Some(String::from("ZoomFit"))
            }
            AppMessage::PdfMessage(PdfMessage::SetLayout(PageLayout::SinglePage)) => {
                Some(String::from("SinglePageLayout"))
            }
            AppMessage::PdfMessage(PdfMessage::SetLayout(PageLayout::DoublePage)) => {
                Some(String::from("DoublePageLayout"))
            }
            AppMessage::PdfMessage(PdfMessage::SetLayout(
                PageLayout::DoublePageTitlePage,
            )) => Some(String::from("DoublePageTitlePageLayout")),
            AppMessage::PdfMessage(PdfMessage::SetLayout(
                PageLayout::Presentation,
            )) => Some(String::from("PresentationLayout")),
            _ => None,
        }
    }

    pub fn bindable(&self) -> Option<BindableMessage> {
        match self {
            AppMessage::OpenNewFileFinder => Some(BindableMessage::OpenFileFinder),
            AppMessage::CloseActiveTab => Some(BindableMessage::CloseTab),
            AppMessage::ToggleDarkModeUi => Some(BindableMessage::ToggleDarkModeUi),
            AppMessage::ToggleDarkModePdf => Some(BindableMessage::ToggleDarkModePdf),
            AppMessage::TogglePageBorders => Some(BindableMessage::TogglePageBorders),
            AppMessage::ToggleSidebar => Some(BindableMessage::ToggleSidebar),
            AppMessage::TogglePresentationMode => {
                Some(BindableMessage::TogglePresentationMode)
            }
            AppMessage::ToggleFullscreen => Some(BindableMessage::ToggleFullscreen),
            AppMessage::PdfMessage(PdfMessage::PrintPdf) => {
                Some(BindableMessage::PrintPdf)
            }
            AppMessage::PdfMessage(PdfMessage::ZoomIn) => {
                Some(BindableMessage::ZoomIn)
            }
            AppMessage::PdfMessage(PdfMessage::ZoomOut) => {
                Some(BindableMessage::ZoomOut)
            }
            AppMessage::PdfMessage(PdfMessage::ZoomHome) => {
                Some(BindableMessage::ZoomHome)
            }
            AppMessage::PdfMessage(PdfMessage::ZoomFit) => {
                Some(BindableMessage::ZoomFit)
            }
            AppMessage::PdfMessage(PdfMessage::SetLayout(PageLayout::SinglePage)) => {
                Some(BindableMessage::SinglePageLayout)
            }
            AppMessage::PdfMessage(PdfMessage::SetLayout(PageLayout::DoublePage)) => {
                Some(BindableMessage::DoublePageLayout)
            }
            AppMessage::PdfMessage(PdfMessage::SetLayout(
                PageLayout::DoublePageTitlePage,
            )) => Some(BindableMessage::DoublePageTitlePageLayout),
            AppMessage::PdfMessage(PdfMessage::SetLayout(
                PageLayout::Presentation,
            )) => Some(BindableMessage::PresentationLayout),
            _ => None,
        }
    }

    pub fn from_menu_id(s: &str) -> Option<Self> {
        match s {
            "OpenFileFinder" => Some(AppMessage::OpenNewFileFinder),
            "CloseActiveTab" => Some(AppMessage::CloseActiveTab),
            "ToggleDarkModeUi" => Some(AppMessage::ToggleDarkModeUi),
            "ToggleDarkModePdf" => Some(AppMessage::ToggleDarkModePdf),
            "TogglePageBorders" => Some(AppMessage::TogglePageBorders),
            "ToggleSidebar" => Some(AppMessage::ToggleSidebar),
            "TogglePresentationMode" => {
                Some(AppMessage::TogglePresentationMode)
            }
            "ToggleFullscreen" => Some(AppMessage::ToggleFullscreen),
            "PrintPdf" => Some(AppMessage::PdfMessage(PdfMessage::PrintPdf)),
            "ZoomIn" => Some(AppMessage::PdfMessage(PdfMessage::ZoomIn)),
            "ZoomOut" => Some(AppMessage::PdfMessage(PdfMessage::ZoomOut)),
            "ZoomHome" => Some(AppMessage::PdfMessage(PdfMessage::ZoomHome)),
            "ZoomFit" => Some(AppMessage::PdfMessage(PdfMessage::ZoomFit)),
            "SinglePageLayout" => Some(AppMessage::PdfMessage(
                PdfMessage::SetLayout(PageLayout::SinglePage),
            )),
            "DoublePageLayout" => Some(AppMessage::PdfMessage(
                PdfMessage::SetLayout(PageLayout::DoublePage),
            )),
            "DoublePageTitlePageLayout" => Some(AppMessage::PdfMessage(
                PdfMessage::SetLayout(PageLayout::DoublePageTitlePage),
            )),
            "PresentationLayout" => Some(AppMessage::PdfMessage(
                PdfMessage::SetLayout(PageLayout::Presentation),
            )),
            _ => None,
        }
    }

    pub fn default_menu_label(&self) -> Option<&'static str> {
        match self {
            AppMessage::OpenNewFileFinder => Some("Open"),
            AppMessage::PdfMessage(PdfMessage::PrintPdf) => Some("Print"),
            AppMessage::CloseActiveTab => Some("Close"),
            AppMessage::ToggleDarkModeUi => Some("Dark Interface"),
            AppMessage::ToggleDarkModePdf => Some("Dark Pdf"),
            AppMessage::TogglePageBorders => Some("Page Borders"),
            AppMessage::PdfMessage(PdfMessage::ZoomIn) => Some("Zoom In"),
            AppMessage::PdfMessage(PdfMessage::ZoomOut) => Some("Zoom Out"),
            AppMessage::PdfMessage(PdfMessage::ZoomHome) => Some("Zoom 100%"),
            AppMessage::PdfMessage(PdfMessage::ZoomFit) => Some("Fit To Screen"),
            AppMessage::ToggleSidebar => Some("Sidebar"),
            AppMessage::TogglePresentationMode => Some("Presentation Mode"),
            AppMessage::ToggleFullscreen => Some("Fullscreen"),
            AppMessage::PdfMessage(PdfMessage::SetLayout(PageLayout::SinglePage)) => {
                Some("Single Page")
            }
            AppMessage::PdfMessage(PdfMessage::SetLayout(PageLayout::DoublePage)) => {
                Some("Double Page")
            }
            AppMessage::PdfMessage(PdfMessage::SetLayout(
                PageLayout::DoublePageTitlePage,
            )) => Some("Double Page w/ Title"),
            AppMessage::PdfMessage(PdfMessage::SetLayout(PageLayout::Presentation)) => {
                Some("Presentation")
            }
            _ => None,
        }
    }
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

    pub fn new(bookmark_store: BookmarkStore, recent_files: RecentFiles) -> Self {
        let cfg = CONFIG.read().unwrap();
        let (mut ps, pdf_id) = pane_grid::State::new(Pane {
            pane_type: PaneType::Pdf,
        });
        if cfg.open_sidebar {
            Self::open_sidebar(&mut ps, pdf_id);
        }

        Self {
            mac_menu: None,
            pdfs: vec![],
            pdf_idx: 0,
            file_watcher: None,
            dark_mode: CONFIG.read().unwrap().dark_mode,
            invert_pdf: CONFIG.read().unwrap().invert_pdf,
            draw_page_borders: CONFIG.read().unwrap().page_borders,
            presentation_mode: false,
            search_open: false,
            search_hover: false,
            bookmark_store,
            recent_files,
            pane_state: ps,
            sidebar_tab: SidebarTab::Outline,
            shift_pressed: false,
            ctrl_pressed: false,
            scale_factor: 1.0,
            jumplist: Jumplist::new(),
        }
    }

    fn open_pdf(&mut self, path_buf: PathBuf) -> iced::Task<AppMessage> {
        let out = match PdfViewer::from_path(path_buf.clone()) {
            Ok(mut viewer) => {
                viewer.set_scale_factor(self.scale_factor);
                viewer.set_pdf_dark_mode(self.invert_pdf);
                self.pdfs.push(viewer);
                iced::Task::done(AppMessage::OpenTab(self.pdfs.len() - 1))
            }
            Err(e) => {
                error!("Couldn't create pdf viewer or {path_buf:?} {e}");
                iced::Task::none()
            }
        };
        if let Some(sender) = self.file_watcher.as_ref() {
            // We should never fill this up from here, thus blocking is alright
            let _ = sender.blocking_send(WatchMessage::StartWatch(path_buf.clone()));
        }
        out
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

    fn open_sidebar(pane_state: &mut pane_grid::State<Pane>, pdf_id: pane_grid::Pane) {
        if let Some((_, split)) = pane_state.split(
            pane_grid::Axis::Vertical,
            pdf_id,
            Pane {
                pane_type: PaneType::Sidebar,
            },
        ) {
            pane_state.resize(split, 0.7);
        }
    }

    pub fn update(&mut self, message: AppMessage) -> iced::Task<AppMessage> {
        let _span = tracy_client::span!("App update");
        match message {
            AppMessage::InitializeMacMenu => {
                let recent_files = self.recent_files.get_recent();
                let m = platform_specific::macos::Menu::new(recent_files);
                m.init();
                self.mac_menu = Some(m);
                iced::Task::none()
            }
            AppMessage::OpenFile(path_buf) => {
                let path_buf = canonicalize(path_buf).unwrap();
                self.recent_files.add_recent(path_buf.clone());
                if let Some(m) = &self.mac_menu {
                    let recent_files = self.recent_files.get_recent();
                    m.update_recent_files(recent_files);
                }
                self.open_pdf(path_buf)
            }
            AppMessage::OpenTempFile(path_buf) => {
                let path_buf = canonicalize(path_buf).unwrap();
                self.open_pdf(path_buf)
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
            AppMessage::PdfMessage(msg) => {
                if !self.pdfs.is_empty() {
                    if self.pdfs[self.pdf_idx].is_jumpable_action(&msg) {
                        self.record_location();
                        let pdf_msg = self.pdfs[self.pdf_idx]
                            .update(msg)
                            .map(AppMessage::PdfMessage);
                        self.record_location();
                        pdf_msg
                    } else {
                        self.pdfs[self.pdf_idx]
                            .update(msg)
                            .map(AppMessage::PdfMessage)
                    }
                } else {
                    iced::Task::none()
                }
            }
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
                if !self.pdfs.is_empty() {
                    self.pdf_idx = (self.pdf_idx + 1).min(self.pdfs.len() - 1);
                }
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
                for pdf in &mut self.pdfs {
                    pdf.set_interface_dark_mode(self.invert_pdf);
                }
                iced::Task::none()
            }
            AppMessage::ToggleDarkModePdf => {
                self.invert_pdf = !self.invert_pdf;
                for pdf in &mut self.pdfs {
                    pdf.set_pdf_dark_mode(self.invert_pdf);
                }
                iced::Task::none()
            }
            AppMessage::TogglePageBorders => {
                self.draw_page_borders = !self.draw_page_borders;
                for pdf in &mut self.pdfs {
                    pdf.draw_page_borders = self.draw_page_borders;
                }
                iced::Task::none()
            }
            AppMessage::None => iced::Task::none(),
            AppMessage::MouseMoved(vector) => {
                if !self.pdfs.is_empty() {
                    self.pdfs[self.pdf_idx]
                        .update(PdfMessage::MouseMoved(vector))
                        .map(AppMessage::PdfMessage)
                } else {
                    iced::Task::none()
                }
            }
            AppMessage::SearchHover(hover) => {
                self.search_hover = hover;
                iced::Task::none()
            }
            AppMessage::MouseButtonDown(button) => {
                if self.search_open && self.search_hover {
                    iced::Task::none()
                } else if !self.pdfs.is_empty()
                    && let Some(action) = self.get_mouse_action(button)
                {
                    self.pdfs[self.pdf_idx]
                        .update(PdfMessage::MouseAction(action, true))
                        .map(AppMessage::PdfMessage)
                } else {
                    iced::Task::none()
                }
            }
            AppMessage::MouseButtonUp(button) => {
                if self.search_open && self.search_hover {
                    iced::Task::none()
                } else if !self.pdfs.is_empty()
                    && let Some(action) = self.get_mouse_action(button)
                {
                    self.pdfs[self.pdf_idx]
                        .update(PdfMessage::MouseAction(action, false))
                        .map(AppMessage::PdfMessage)
                } else {
                    iced::Task::none()
                }
            }
            AppMessage::ShiftPressed(pressed) => {
                self.shift_pressed = pressed;
                iced::Task::none()
            }
            AppMessage::CtrlPressed(pressed) => {
                self.ctrl_pressed = pressed;
                iced::Task::none()
            }
            AppMessage::ModifiersChanged(modifiers) => {
                self.shift_pressed = modifiers.shift();
                self.ctrl_pressed = modifiers.control();
                iced::Task::none()
            }
            AppMessage::BookmarkMessage(BookmarkMessage::RequestNewBookmark { name }) => {
                let path = self.pdfs.get(self.pdf_idx).map(|pdf| pdf.path.clone());
                let page = self.pdfs.get(self.pdf_idx).map(|pdf| pdf.current_page());
                if let (Some(path), Some(page)) = (path, page) {
                    self.bookmark_store
                        .update(BookmarkMessage::CreateBookmark { path, name, page })
                        .map(AppMessage::BookmarkMessage)
                } else {
                    iced::Task::none()
                }
            }
            AppMessage::BookmarkMessage(BookmarkMessage::GoTo { path, page }) => {
                if let Some(pdf_index) = self.pdfs.iter().position(|pdf| pdf.path == path) {
                    self.record_location();
                    self.pdf_idx = pdf_index;
                    let pdf_msg = self.pdfs[pdf_index]
                        .update(PdfMessage::SetPage(page))
                        .map(AppMessage::PdfMessage);
                    self.record_location();
                    return pdf_msg;
                }
                iced::Task::done(AppMessage::OpenFile(path.clone())).chain(iced::Task::done(
                    AppMessage::BookmarkMessage(BookmarkMessage::GoTo {
                        path: path.clone(),
                        page,
                    }),
                ))
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
                } else if let Some(pdf_id) = self.get_pdf_pane_id() {
                    Self::open_sidebar(&mut self.pane_state, pdf_id);
                }
                iced::Task::none()
            }
            AppMessage::SetSidebar(sidebar_tab) => {
                self.sidebar_tab = sidebar_tab;
                iced::Task::none()
            }
            AppMessage::OutlineGoToPage(page) => {
                if !self.pdfs.is_empty() {
                    self.record_location();
                    let pdf_msg = self.pdfs[self.pdf_idx]
                        .update(PdfMessage::SetPage(page))
                        .map(AppMessage::PdfMessage);
                    self.record_location();
                    pdf_msg
                } else {
                    iced::Task::none()
                }
            }
            AppMessage::CloseActiveTab => iced::Task::done(AppMessage::CloseTab(self.pdf_idx)),
            AppMessage::Scroll(delta) => {
                if !self.pdfs.is_empty() {
                    match delta {
                        iced::mouse::ScrollDelta::Lines { y, .. } => {
                            let button = if y > 0.0 {
                                MouseButton::ScrollUp
                            } else if y < 0.0 {
                                MouseButton::ScrollDown
                            } else {
                                return iced::Task::none();
                            };
                            if let Some(action) = self.get_mouse_action(button) {
                                self.pdfs[self.pdf_idx]
                                    .update(PdfMessage::MouseAction(action, true))
                                    .map(AppMessage::PdfMessage)
                            } else {
                                iced::Task::none()
                            }
                        }
                        iced::mouse::ScrollDelta::Pixels { x, y } => {
                            let sensitivity = CONFIG.read().unwrap().trackpad_sensitivity;
                            let move_vec = Vector::new(-x * sensitivity, y * sensitivity);
                            self.pdfs[self.pdf_idx]
                                .update(PdfMessage::Move(move_vec))
                                .map(AppMessage::PdfMessage)
                        }
                    }
                } else {
                    iced::Task::none()
                }
            }
            AppMessage::Exit => exit(),
            AppMessage::FoundWindowId(id) => match id {
                Some(id) => iced::window::scale_factor(id)
                    .map(AppMessage::FoundScaleFactor)
                    .chain(iced::Task::none()),
                None => iced::Task::none(),
            },
            AppMessage::FoundScaleFactor(scale) => {
                self.scale_factor = scale as f64;
                for viewer in &mut self.pdfs {
                    viewer.set_scale_factor(self.scale_factor);
                }
                iced::Task::none()
            }
            AppMessage::JumpTo(location) => {
                match self
                    .pdfs
                    .iter_mut()
                    .enumerate()
                    .find(|(_, pdf)| pdf.path == location.pdf_path)
                {
                    Some((i, pdf)) => {
                        self.pdf_idx = i;
                        pdf.update(PdfMessage::SetLocation(
                            location.translation,
                            location.scale,
                        ))
                        .map(AppMessage::PdfMessage)
                    }
                    None => iced::Task::done(AppMessage::OpenFile(location.pdf_path.clone()))
                        .chain(iced::Task::done(AppMessage::JumpTo(location))),
                }
            }
            AppMessage::JumpBack => {
                if let Some(location) = self.jumplist.jump_back() {
                    return iced::Task::done(AppMessage::JumpTo(location.clone()));
                }
                iced::Task::none()
            }
            AppMessage::JumpForward => {
                if let Some(location) = self.jumplist.jump_forward() {
                    return iced::Task::done(AppMessage::JumpTo(location.clone()));
                }
                iced::Task::none()
            }
            AppMessage::ToggleFullscreen => toggle_fullscreen(),
            AppMessage::TogglePresentationMode => {
                self.presentation_mode = !self.presentation_mode;
                iced::Task::none()
            }
            AppMessage::OpenSearch => {
                self.search_open = true;
                let search_task = if !self.pdfs.is_empty() {
                    self.pdfs[self.pdf_idx]
                        .update(PdfMessage::HighlightSearchResults)
                        .map(AppMessage::PdfMessage)
                } else {
                    iced::Task::none()
                };
                if self.search_open {
                    iced::Task::batch([
                        search_task,
                        widget::operation::focus(widget::Id::new("search_input"))
                            .map(|_: ()| AppMessage::PdfMessage(PdfMessage::None)),
                    ])
                } else {
                    search_task
                }
            }
            AppMessage::CloseSearch => {
                if self.search_open {
                    self.search_open = false;
                    if !self.pdfs.is_empty() {
                        self.pdfs[self.pdf_idx]
                            .update(PdfMessage::HideSearchResults)
                            .map(AppMessage::PdfMessage)
                    } else {
                        iced::Task::none()
                    }
                } else {
                    iced::Task::none()
                }
            }
            AppMessage::ToggleSearchMethod => {
                if self.search_open {
                    if let Some(viewer) = self.pdfs.get_mut(self.pdf_idx) {
                        viewer
                            .update(PdfMessage::ToggleSearchMethod)
                            .map(AppMessage::PdfMessage)
                    } else {
                        iced::Task::none()
                    }
                } else {
                    iced::Task::none()
                }
            }
        }
    }

    fn record_location(&mut self) {
        if let Some(pdf) = self.pdfs.get(self.pdf_idx) {
            self.jumplist.push(JumpLocation {
                pdf_path: pdf.path.clone(),
                translation: pdf.translation,
                scale: pdf.scale,
            })
        };
    }

    fn create_tabs(&self) -> Element<'_, AppMessage> {
        let mut command_bar = widget::Row::new();
        for (i, pdf) in self.pdfs.iter().enumerate() {
            command_bar = command_bar.push(file_tab(
                &pdf.name,
                pdf.page_progress(),
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

    fn search_view(&self) -> Element<'_, AppMessage> {
        let search_method = self.pdfs.get(self.pdf_idx).map(|x| x.search_method);
        let search_progress = self
            .pdfs
            .get(self.pdf_idx)
            .map(|x| x.search_progress())
            .unwrap_or_default();
        widget::row![
            widget::space::horizontal().width(Length::Fill),
            widget::mouse_area(
                widget::container(
                    widget::column![
                        widget::text_input(
                            "Search",
                            self.pdfs
                                .get(self.pdf_idx)
                                .map(|x| x.needle.as_str())
                                .unwrap_or("")
                        )
                        .id(widget::Id::new("search_input"))
                        .on_input(|x| AppMessage::PdfMessage(PdfMessage::UpdateSearchNeedle(x))),
                        widget::row![
                            widget::button("Plain text")
                                .style(move |theme, status| Self::search_method_button_style(
                                    theme,
                                    status,
                                    search_method == Some(SearchMethod::PlainText)
                                ))
                                .on_press(
                                    PdfMessage::SetSearchMethod(SearchMethod::PlainText).into()
                                ),
                            widget::button("Regex")
                                .style(move |theme, status| Self::search_method_button_style(
                                    theme,
                                    status,
                                    search_method == Some(SearchMethod::Regex)
                                ))
                                .on_press(PdfMessage::SetSearchMethod(SearchMethod::Regex).into()),
                            widget::space::horizontal().width(Length::Fill),
                            widget::text(search_progress),
                        ]
                        .align_y(alignment::Vertical::Center)
                        .spacing(4.0),
                    ]
                    .spacing(4.0)
                )
                .padding(8.0)
                .style(|theme: &Theme| widget::container::Style {
                    background: Some(theme.extended_palette().background.weak.color.into()),
                    border: Border {
                        color: theme.extended_palette().primary.base.color,
                        width: 2.0,
                        radius: Radius::from(8.0),
                    },
                    shadow: Shadow {
                        color: theme.extended_palette().primary.base.color,
                        offset: iced::Vector { x: 0.0, y: 2.0 },
                        blur_radius: 4.0,
                    },
                    ..Default::default()
                })
            )
            .on_enter(AppMessage::SearchHover(true))
            .on_exit(AppMessage::SearchHover(false))
            .on_press(AppMessage::None)
        ]
        .into()
    }

    fn search_method_button_style(
        theme: &Theme,
        _status: widget::button::Status,
        selected: bool,
    ) -> widget::button::Style {
        let palette = theme.extended_palette();
        if selected {
            button::Style {
                background: Some(palette.primary.base.color.into()),
                text_color: palette.primary.base.text,
                border: Border {
                    radius: Radius::from(4.0),
                    ..Default::default()
                },
                ..Default::default()
            }
        } else {
            button::Style {
                background: Some(palette.secondary.base.color.into()),
                text_color: palette.secondary.base.text,
                border: Border {
                    radius: Radius::from(4.0),
                    ..Default::default()
                },
                ..Default::default()
            }
        }
    }

    pub fn view(&self) -> iced::Element<'_, AppMessage> {
        let pg = PaneGrid::new(&self.pane_state, |_id, pane, _is_maximized| {
            pane_grid::Content::new(match pane.pane_type {
                PaneType::Sidebar => self.view_sidebar(),
                PaneType::Pdf => {
                    let pdf_content: iced::Element<'_, AppMessage> = if self.pdfs.is_empty() {
                        widget::space::vertical().into()
                    } else {
                        self.pdfs[self.pdf_idx].view().map(AppMessage::PdfMessage)
                    };
                    let tabs = self.create_tabs();
                    if self.presentation_mode {
                        widget::column![stack![pdf_content,]].into()
                    } else {
                        let mut stack_children: Vec<Element<'_, AppMessage>> = vec![
                            pdf_content,
                            container(tabs)
                                .align_y(alignment::Vertical::Bottom)
                                .width(Length::Fill)
                                .height(Length::Fill)
                                .padding(8.0)
                                .into(),
                        ];
                        if self.search_open {
                            stack_children.push(
                                container(self.search_view())
                                    .align_x(alignment::Horizontal::Right)
                                    .align_y(alignment::Vertical::Top)
                                    .width(Length::Fill)
                                    .padding(8.0)
                                    .into(),
                            );
                        }
                        if self.mac_menu.is_none() {
                            let menu_bar = platform_specific::iced_aw::create_menu_bar(
                                self.pdfs.is_empty(),
                                self.has_sidebar_pane(),
                                self.dark_mode,
                                self.invert_pdf,
                                self.draw_page_borders,
                                self.pdf_idx,
                                &self.recent_files.get_recent(),
                            );
                            widget::column![menu_bar, stack(stack_children)].into()
                        } else {
                            widget::column![stack(stack_children)].into()
                        }
                    }
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
                    widget::space::horizontal().width(8.0),
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
                        _ => Some(palette.primary.base.color.into()),
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
                        _ => Some(palette.primary.base.color.into()),
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
            widget::space::vertical().height(8.0),
            contents,
        ]
        .padding(8.0)
        .into()
    }

    fn view_outline(&self) -> Element<'_, AppMessage> {
        let mut col = widget::column![
            text("Document Outline").size(18.0),
            widget::space::vertical().height(8.0),
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
                    Some(AppMessage::ModifiersChanged(modifiers))
                }
                iced::keyboard::Event::KeyPressed {
                    key: _,
                    modified_key: iced::keyboard::Key::Character(ref modified),
                    physical_key: _,
                    location: _,
                    modifiers: _,
                    text: _,
                    repeat: _,
                } => {
                    let e = if modified == "+" {
                        iced::keyboard::Event::KeyPressed {
                            key: iced::keyboard::Key::Character(SmolStr::new_static("+")),
                            modified_key: iced::keyboard::Key::Character(SmolStr::new_static("+")),
                            physical_key: iced::keyboard::key::Physical::Code(
                                iced::keyboard::key::Code::Minus,
                            ),
                            location: iced::keyboard::Location::Standard,
                            modifiers: Modifiers::empty(),
                            text: Some(SmolStr::new_static("+")),
                            repeat: false,
                        }
                    } else {
                        keyboard_event
                    };
                    let mut config = CONFIG.write().unwrap();
                    match status {
                        iced::event::Status::Ignored => {
                            config.keyboard.dispatch(e).map(|x| (*x).into())
                        }
                        iced::event::Status::Captured => None,
                    }
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
                iced::mouse::Event::ButtonPressed(button) => {
                    iced_to_config_mouse_button(button).map(AppMessage::MouseButtonDown)
                }
                iced::mouse::Event::ButtonReleased(button) => {
                    iced_to_config_mouse_button(button).map(AppMessage::MouseButtonUp)
                }
                iced::mouse::Event::WheelScrolled { delta } => match status {
                    iced::event::Status::Ignored => match delta {
                        iced::mouse::ScrollDelta::Lines { x: _, y: _ } => {
                            Some(AppMessage::Scroll(delta))
                        }
                        iced::mouse::ScrollDelta::Pixels { x: _, y: _ } => {
                            Some(AppMessage::Scroll(delta))
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
        subs.append(&mut platform_specific::listeners());

        let config = CONFIG.read().unwrap();
        if config.rpc_enabled {
            subs.push(Subscription::run(rpc_server));
        }

        Subscription::batch(subs)
    }
}

impl Drop for App {
    fn drop(&mut self) {
        match self.bookmark_store.save() {
            Ok(_) => {}
            Err(e) => {
                error!("Error while saving bookmarks: {}", e)
            }
        }
        match self.recent_files.save() {
            Ok(_) => {}
            Err(e) => {
                error!("Error while saving recent files: {}", e)
            }
        }
    }
}

fn view_outline_items<'a>(items: &'a [OutlineItem], level: u32) -> widget::Column<'a, AppMessage> {
    let mut col = widget::column![];

    for item in items {
        let indent = level * 16; // 16 pixels per level

        let item_button = if let Some(page) = item.page {
            button(
                container(text(&item.title).shaping(text::Shaping::Advanced).style(
                    |theme: &Theme| {
                        let palette = theme.extended_palette();
                        text::Style {
                            color: Some(palette.primary.base.color),
                        }
                    },
                ))
                .padding(Padding::default().left(indent as f32)),
            )
            .style(|_: &Theme, _| widget::button::Style {
                background: None,
                ..Default::default()
            })
            .width(Length::Fill)
            .on_press(AppMessage::OutlineGoToPage(page as usize))
        } else {
            button(
                container(text(&item.title).shaping(text::Shaping::Advanced).style(
                    |theme: &Theme| {
                        let palette = theme.extended_palette();
                        text::Style {
                            color: Some(palette.background.weak.color),
                        }
                    },
                ))
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

pub(crate) fn base_button<'a>(
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

#[allow(dead_code)]
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

fn file_tab<'a>(
    file_name: &'a str,
    page_progress: String,
    on_press: AppMessage,
    on_close: AppMessage,
    is_open: bool,
) -> Element<'a, AppMessage> {
    container(
        widget::row![
            base_button(
                widget::row![
                    text(file_name)
                        .font(Font {
                            family: iced::font::Family::Name("Geist"),
                            weight: Weight::Semibold,
                            ..Default::default()
                        })
                        .shaping(text::Shaping::Advanced),
                    text(page_progress).shaping(text::Shaping::Advanced)
                ]
                .spacing(8.0),
                on_press
            )
            .style(file_tab_style),
            // TODO: Svg X
            base_button(
                text("×").align_y(alignment::Vertical::Bottom).size(24.0),
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
            snap: true,
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

fn toggle_fullscreen() -> iced::Task<AppMessage> {
    iced::window::latest()
        .and_then(move |id| iced::window::mode(id).map(move |mode| (id, mode)))
        .then(|(id, current_mode)| match current_mode {
            window::Mode::Fullscreen => iced::window::set_mode(id, window::Mode::Windowed),
            _ => iced::window::set_mode(id, window::Mode::Fullscreen),
        })
}

fn iced_to_config_mouse_button(button: iced::mouse::Button) -> Option<MouseButton> {
    match button {
        iced::mouse::Button::Left => Some(MouseButton::Left),
        iced::mouse::Button::Right => Some(MouseButton::Right),
        iced::mouse::Button::Middle => Some(MouseButton::Middle),
        iced::mouse::Button::Back => Some(MouseButton::Back),
        iced::mouse::Button::Forward => Some(MouseButton::Forward),
        iced::mouse::Button::Other(_) => None,
    }
}
