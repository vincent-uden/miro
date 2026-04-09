use std::{fs, path::PathBuf};

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};

const MAX_RECENT_FILES: usize = 5;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RecentFiles {
    files: Vec<PathBuf>,
}

impl RecentFiles {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn system_store() -> Result<Self> {
        let path = Self::system_store_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        serde_json::from_str(&fs::read_to_string(path)?).map_err(|e| anyhow!("{}", e))
    }

    fn system_store_path() -> Result<PathBuf> {
        Ok(home::home_dir()
            .ok_or(anyhow!("No home directory could be determined"))?
            .join("./.config/miro-pdf/recent_files.json"))
    }

    pub fn save(&self) -> Result<()> {
        fs::write(
            Self::system_store_path()?,
            serde_json::to_string(self).map_err(|e| anyhow!("{}", e))?,
        )
        .map_err(|e| anyhow!("{}", e))
    }

    pub fn add_recent(&mut self, path: PathBuf) {
        self.files.retain(|p| p != &path);
        self.files.insert(0, path);
        if self.files.len() > MAX_RECENT_FILES {
            self.files.truncate(MAX_RECENT_FILES);
        }
    }

    pub fn get_recent(&self) -> &[PathBuf] {
        &self.files
    }
}
