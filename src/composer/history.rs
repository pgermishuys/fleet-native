//! The last texts sent in a session, for ArrowUp on an empty draft (`use-input-history.ts`). Newest first,
//! trimmed, de-duplicated, at most ten. The open list and its highlight are here; the popup is piece M.

pub const MAX_HISTORY: usize = 10;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct History {
    /// Newest first.
    pub entries: Vec<String>,
    pub open: bool,
    /// The highlighted entry; `None` while closed.
    pub selected: Option<usize>,
}

impl History {
    /// A history read from the file: kept as it is, at most ten.
    pub fn from_entries(mut entries: Vec<String>) -> Self {
        entries.truncate(MAX_HISTORY);
        History { entries, ..Default::default() }
    }

    /// Records a sent text: trimmed, a repeat moves to the front, the oldest beyond ten drops. Empty is ignored.
    /// Returns whether the list changed.
    pub fn push(&mut self, text: &str) -> bool {
        let text = text.trim();
        if text.is_empty() {
            return false;
        }
        self.entries.retain(|e| e != text);
        self.entries.insert(0, text.to_string());
        self.entries.truncate(MAX_HISTORY);
        true
    }

    /// Opens the list on the newest entry; nothing happens with no entries.
    pub fn open(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        self.open = true;
        self.selected = Some(0);
    }

    pub fn close(&mut self) {
        self.open = false;
        self.selected = None;
    }

    /// ArrowUp while open: towards an older entry (clamped, no wrap). Fleet's `moveDown`: its index counts from
    /// the newest, and the list is drawn oldest at the top, so Up there is a higher index.
    pub fn older(&mut self) {
        if let (true, Some(i)) = (self.open, self.selected)
            && i + 1 < self.entries.len()
        {
            self.selected = Some(i + 1);
        }
    }

    /// ArrowDown while open: towards a newer entry (clamped). Fleet's `moveUp`.
    pub fn newer(&mut self) {
        if let (true, Some(i)) = (self.open, self.selected)
            && i > 0
        {
            self.selected = Some(i - 1);
        }
    }

    /// The highlighted entry, closing the list. `None` when closed.
    pub fn confirm(&mut self) -> Option<String> {
        let entry = self.selected.filter(|_| self.open).and_then(|i| self.entries.get(i).cloned());
        if self.open && self.selected.is_some() {
            self.close();
        }
        entry
    }
}
