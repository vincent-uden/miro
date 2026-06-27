use crate::config::BindableMessage;

pub enum CommonMenuItem {
    Button(BindableMessage),
    RecentFiles,
    Separator,
}
pub fn items() -> Vec<(String, Vec<CommonMenuItem>)> {
    vec![
        (
            String::from("File"),
            vec![
                CommonMenuItem::Button(BindableMessage::OpenFileFinder),
                CommonMenuItem::Button(BindableMessage::PrintPdf),
                CommonMenuItem::Separator,
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
                CommonMenuItem::Button(BindableMessage::ToggleSidebar),
                CommonMenuItem::Separator,
                CommonMenuItem::Button(BindableMessage::ZoomIn),
                CommonMenuItem::Button(BindableMessage::ZoomOut),
                CommonMenuItem::Button(BindableMessage::ZoomHome),
                CommonMenuItem::Button(BindableMessage::ZoomFit),
                CommonMenuItem::Separator,
                CommonMenuItem::Button(BindableMessage::TogglePresentationMode),
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
    ]
}
