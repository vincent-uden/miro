use crate::{config::BindableMessage};

pub enum CommonMenuItem {
    Button(String, BindableMessage),
    RecentFiles,
    Separator,
}
pub struct CommonMenu {
    pub skeleton: Vec<(String, Vec<CommonMenuItem>)>,
}

impl CommonMenu {
    pub fn new() -> Self {
        let skeleton = vec![(
            String::from("File"),
            vec![
                CommonMenuItem::Button(String::from("Open File"), BindableMessage::OpenFileFinder),
                CommonMenuItem::Button(String::from("Print PDF"), BindableMessage::PrintPdf),
                CommonMenuItem::RecentFiles,
            ],
        )];
        Self { skeleton: skeleton }
    }
}
