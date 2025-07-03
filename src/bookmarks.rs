use std::{fs, path::PathBuf};

use anyhow::{Result, anyhow};
use iced::{
    Length,
    widget::{container, text},
};
use serde::{Deserialize, Serialize};
use strum::EnumString;
use twox_hash::XxHash64;

// This does not need to be cryptographically sound in the slightest. It is just used for
// fingerprinting files to detect updates.
const HASH_SEED: usize = 1337;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bookmark {
    page: usize,
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BookmarkSet {
    marks: Vec<Bookmark>,
    file_hash: XxHash64,
    path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, EnumString, Default)]
pub enum BookmarkMessage {
    CreateBookmark {
        path: PathBuf,
        name: String,
        page: usize,
    },
    DeleteBookmark {
        path: PathBuf,
        name: String,
        page: usize,
    },
    GoTo {
        path: PathBuf,
        page: usize,
    },
    #[default]
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookmarkStore {
    sets: Vec<BookmarkSet>,
}

impl BookmarkStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn system_store() -> Result<Self> {
        serde_json::from_str(&fs::read_to_string(Self::system_store_path()?)?)
            .map_err(|e| anyhow!("{}", e))
    }

    fn system_store_path() -> Result<PathBuf> {
        Ok(home::home_dir()
            .ok_or(anyhow!("No home directory could be determined"))?
            .join("./.config/miro-pdf/bookmarks.json"))
    }

    pub fn save(&self) -> Result<()> {
        fs::write(
            Self::system_store_path()?,
            serde_json::to_string(self).map_err(|e| anyhow!("{}", e))?,
        )
        .map_err(|e| anyhow!("{}", e))
    }

    pub fn update(&mut self, message: BookmarkMessage) -> iced::Task<BookmarkMessage> {
        match message {
            BookmarkMessage::CreateBookmark { path, name, page } => todo!(),
            BookmarkMessage::DeleteBookmark { path, name, page } => todo!(),
            BookmarkMessage::GoTo { path, page } => panic!("Should be handled by app"),
            BookmarkMessage::None => todo!(),
        }
    }

    pub fn view(&self) -> iced::Element<'_, BookmarkMessage> {
        container(text("Bookmarks")).height(Length::Fill).into()
    }
}

impl Default for BookmarkStore {
    fn default() -> Self {
        Self { sets: vec![] }
    }
}
