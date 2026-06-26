use crate::app::AppMessage;
use crate::pdf::PdfMessage;
use crate::pdf::page_layout::PageLayout;

pub enum CommonMenuItem {
    Button(String, AppMessage),
    RecentFiles,
    Separator,
}
pub struct CommonMenu {
    pub skeleton: Vec<(String, Vec<CommonMenuItem>)>,
}

impl CommonMenu {
    pub fn new() -> Self {
        let skeleton = vec![
            (
                String::from("File"),
                vec![
                    CommonMenuItem::Button(String::from("Open"), AppMessage::OpenNewFileFinder),
                    CommonMenuItem::RecentFiles,
                    CommonMenuItem::Separator,
                    CommonMenuItem::Button(
                        String::from("Print"),
                        AppMessage::PdfMessage(PdfMessage::PrintPdf),
                    ),
                    CommonMenuItem::Button(String::from("Close"), AppMessage::CloseActiveTab),
                ],
            ),
            (
                String::from("View"),
                vec![
                    CommonMenuItem::Button(
                        String::from("Dark Interface"),
                        AppMessage::ToggleDarkModeUi,
                    ),
                    CommonMenuItem::Button(String::from("Dark Pdf"), AppMessage::ToggleDarkModePdf),
                    CommonMenuItem::Button(
                        String::from("Page Borders"),
                        AppMessage::TogglePageBorders,
                    ),
                    CommonMenuItem::Button(
                        String::from("Zoom In"),
                        AppMessage::PdfMessage(PdfMessage::ZoomIn),
                    ),
                    CommonMenuItem::Button(
                        String::from("Zoom Out"),
                        AppMessage::PdfMessage(PdfMessage::ZoomOut),
                    ),
                    CommonMenuItem::Button(
                        String::from("Zoom 100%"),
                        AppMessage::PdfMessage(PdfMessage::ZoomHome),
                    ),
                    CommonMenuItem::Button(
                        String::from("Fit To Screen"),
                        AppMessage::PdfMessage(PdfMessage::ZoomFit),
                    ),
                    CommonMenuItem::Button(String::from("Sidebar"), AppMessage::ToggleSidebar),
                    CommonMenuItem::Button(
                        String::from("Presentation Mode"),
                        AppMessage::TogglePresentationMode,
                    ),
                    CommonMenuItem::Separator,
                    CommonMenuItem::Button(
                        String::from("Fullscreen"),
                        AppMessage::ToggleFullscreen,
                    ),
                ],
            ),
            (
                String::from("Layout"),
                vec![
                    CommonMenuItem::Button(
                        String::from("Single Page"),
                        AppMessage::PdfMessage(PdfMessage::SetLayout(PageLayout::SinglePage)),
                    ),
                    CommonMenuItem::Button(
                        String::from("Double Page"),
                        AppMessage::PdfMessage(PdfMessage::SetLayout(PageLayout::DoublePage)),
                    ),
                    CommonMenuItem::Button(
                        String::from("Double Page w/ Title"),
                        AppMessage::PdfMessage(PdfMessage::SetLayout(
                            PageLayout::DoublePageTitlePage,
                        )),
                    ),
                    CommonMenuItem::Button(
                        String::from("Presentation"),
                        AppMessage::PdfMessage(PdfMessage::SetLayout(PageLayout::Presentation)),
                    ),
                ],
            ),
        ];
        Self { skeleton }
    }
}
