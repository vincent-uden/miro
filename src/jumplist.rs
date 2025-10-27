use serde::{Deserialize, Serialize};

use crate::geometry::Vector;
use std::{collections::VecDeque, path::PathBuf};

const LOCATION_TOLERANCE_PX: f32 = 5.0;
const JUMPLIST_CAPACITY: usize = 100;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct JumpLocation {
    pub pdf_path: PathBuf,
    pub page: i32,
    pub translation: Vector<f32>,
}

impl JumpLocation {
    pub fn approx_equal(&self, other: &Self) -> bool {
        if self.pdf_path != other.pdf_path || self.page != other.page {
            return false;
        }

        // Check translation within 5px tolerance
        let dx = (self.translation.x - other.translation.x).abs();
        let dy = (self.translation.y - other.translation.y).abs();
        dx < LOCATION_TOLERANCE_PX && dy < LOCATION_TOLERANCE_PX
    }
}

#[derive(Debug)]
pub struct Jumplist {
    pub entries: VecDeque<JumpLocation>,
    pub current_index: usize,
}

impl Jumplist {
    pub fn new() -> Self {
        Jumplist {
            entries: VecDeque::with_capacity(JUMPLIST_CAPACITY),
            current_index: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn push(&mut self, location: JumpLocation) {
        // Skip if duplicate of current position
        if let Some(current) = self.entries.get(self.current_index)
            && current.approx_equal(&location)
        {
            return;
        }

        // Truncate forward history
        if !self.is_empty() {
            self.entries.truncate(self.current_index + 1);
        }
        if self.len() == JUMPLIST_CAPACITY {
            self.entries.pop_front();
            self.current_index -= 1
        }

        self.entries.push_back(location);
        self.current_index = self.len() - 1;
    }

    pub fn jump_back(&mut self) -> Option<&JumpLocation> {
        if self.current_index == 0 {
            return None;
        }
        self.current_index -= 1;
        self.entries.get(self.current_index)
    }

    pub fn jump_forward(&mut self) -> Option<&JumpLocation> {
        if self.len() <= 1 || self.current_index >= self.len() - 1 {
            return None;
        }
        self.current_index += 1;
        self.entries.get(self.current_index)
    }
}

impl Default for Jumplist {
    fn default() -> Self {
        Self::new()
    }
}
