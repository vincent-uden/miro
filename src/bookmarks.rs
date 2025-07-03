use std::{fs, path::PathBuf};

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use twox_hash::XxHash64;

// This does not need to be cryptographically sound in the slightest. It is just used for
// fingerprinting files to detect updates.
const HASH_SEED: usize = 1337;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Bookmark {
    page: usize,
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BookmarkSet {
    marks: Vec<Bookmark>,
    file_hash: XxHash64,
    path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BookmarkStore {
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
}

impl Default for BookmarkStore {
    fn default() -> Self {
        Self { sets: vec![] }
    }
}
