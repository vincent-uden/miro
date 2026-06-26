use crate::app::AppMessage;
use crate::pdf::PdfMessage;
use crate::pdf::page_layout::PageLayout;

pub enum CommonMenuItem {
    Button(AppMessage),
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
                    CommonMenuItem::Button(AppMessage::OpenNewFileFinder),
                    CommonMenuItem::Button(AppMessage::PdfMessage(PdfMessage::PrintPdf)),
                    CommonMenuItem::RecentFiles,
                    CommonMenuItem::Separator,
                    CommonMenuItem::Button(AppMessage::CloseActiveTab),
                ],
            ),
            (
                String::from("View"),
                vec![
                    CommonMenuItem::Button(AppMessage::ToggleDarkModeUi),
                    CommonMenuItem::Button(AppMessage::ToggleDarkModePdf),
                    CommonMenuItem::Button(AppMessage::TogglePageBorders),
                    CommonMenuItem::Button(AppMessage::PdfMessage(PdfMessage::ZoomIn)),
                    CommonMenuItem::Button(AppMessage::PdfMessage(PdfMessage::ZoomOut)),
                    CommonMenuItem::Button(AppMessage::PdfMessage(PdfMessage::ZoomHome)),
                    CommonMenuItem::Button(AppMessage::PdfMessage(PdfMessage::ZoomFit)),
                    CommonMenuItem::Button(AppMessage::ToggleSidebar),
                    CommonMenuItem::Button(AppMessage::TogglePresentationMode),
                    CommonMenuItem::Separator,
                    CommonMenuItem::Button(AppMessage::ToggleFullscreen),
                ],
            ),
            (
                String::from("Layout"),
                vec![
                    CommonMenuItem::Button(AppMessage::PdfMessage(PdfMessage::SetLayout(
                        PageLayout::SinglePage,
                    ))),
                    CommonMenuItem::Button(AppMessage::PdfMessage(PdfMessage::SetLayout(
                        PageLayout::DoublePage,
                    ))),
                    CommonMenuItem::Button(AppMessage::PdfMessage(PdfMessage::SetLayout(
                        PageLayout::DoublePageTitlePage,
                    ))),
                    CommonMenuItem::Button(AppMessage::PdfMessage(PdfMessage::SetLayout(
                        PageLayout::Presentation,
                    ))),
                ],
            ),
        ];
        Self { skeleton }
    }
}
