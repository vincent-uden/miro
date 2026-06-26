use crate::config::BindableMessage;

pub enum CommonMenuItem {
    Button(BindableMessage),
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
                    CommonMenuItem::Button(BindableMessage::OpenFileFinder),
                    CommonMenuItem::Button(BindableMessage::PrintPdf),
                    CommonMenuItem::RecentFiles,
                    CommonMenuItem::Separator,
                    CommonMenuItem::Button(BindableMessage::CloseTab),
                ],
            ),
            (
                String::from("View"),
                vec![
                    CommonMenuItem::Button(BindableMessage::ToggleDarkModeUi),
                    CommonMenuItem::Button(BindableMessage::ToggleDarkModePdf),
                    CommonMenuItem::Button(BindableMessage::TogglePageBorders),
                    CommonMenuItem::Button(BindableMessage::ZoomIn),
                    CommonMenuItem::Button(BindableMessage::ZoomOut),
                    CommonMenuItem::Button(BindableMessage::ZoomHome),
                    CommonMenuItem::Button(BindableMessage::ZoomFit),
                    CommonMenuItem::Button(BindableMessage::ToggleSidebar),
                    CommonMenuItem::Button(BindableMessage::TogglePresentationMode),
                    CommonMenuItem::Separator,
                    CommonMenuItem::Button(BindableMessage::ToggleFullscreen),
                ],
            ),
            (
                String::from("Layout"),
                vec![
                    CommonMenuItem::Button(BindableMessage::SinglePageLayout),
                    CommonMenuItem::Button(BindableMessage::DoublePageLayout),
                    CommonMenuItem::Button(BindableMessage::DoublePageTitlePageLayout),
                    CommonMenuItem::Button(BindableMessage::PresentationLayout),
                ],
            ),
        ];
        Self { skeleton }
    }
}
