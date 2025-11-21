use crate::data::{self, Snapshot};
use ratatui::widgets::TableState;
use std::sync::mpsc::{self, Receiver, Sender};
use std::collections::HashSet;
use tachyonfx::Effect;

pub enum SortKey {
    Number,
    Type,
    Date,
    User,
    UsedSpace,
}

pub struct App {
    pub snapshots: Vec<Snapshot>,
    pub table_state: TableState,
    pub message: String,
    pub loading: bool,
    pub status_text: String,
    pub details_scroll: u16,
    pub status_scroll: u16,
    pub spinner_state: usize,
    pub spinner_frames: Vec<&'static str>,
    pub show_delete_popup: bool,
    pub show_apply_popup: bool,
    pub show_splash: bool,
    pub splash_start: Option<std::time::Instant>,
    pub fx: Option<Effect>,
    pub fx_start: Option<std::time::Instant>,
    pub current_sort_key: SortKey,
    pub sort_ascending: bool,
    pub rx: Option<Receiver<Result<Vec<Snapshot>, String>>>,
    pub selected_indices: HashSet<usize>,
}

impl App {
    pub fn new() -> App {
        App {
            snapshots: Vec::new(),
            table_state: TableState::default(),
            message: String::from("Initializing..."),
            loading: true, // Start loading immediately
            status_text: String::new(),
            details_scroll: 0,
            status_scroll: 0,
            spinner_state: 0,
            spinner_frames: vec!["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"],
            show_delete_popup: false,
            show_apply_popup: false,
            show_splash: true,
            splash_start: Some(std::time::Instant::now()),
            fx: None,
            fx_start: None,
            current_sort_key: SortKey::Number,
            sort_ascending: true,
            rx: None,
            selected_indices: HashSet::new(),
        }
    }

    pub fn refresh_snapshots(&mut self) {
        self.loading = true;
        self.message = String::from("Fetching snapshots...");
        
        match data::list_snapshots() {
            Ok(snapshots) => {
                self.snapshots = snapshots;
                self.sort_snapshots();
                self.loading = false;
                self.message = String::from("Snapshots loaded.");
                if !self.snapshots.is_empty() {
                    self.table_state.select(Some(0));
                }
            }
            Err(e) => {
                self.loading = false;
                self.message = format!("Error: {}", e);
            }
        }
    }

    pub fn next(&mut self) {
        let i = match self.table_state.selected() {
            Some(i) => {
                if i >= self.snapshots.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.table_state.select(Some(i));
    }

    pub fn previous(&mut self) {
        let i = match self.table_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.snapshots.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.table_state.select(Some(i));
    }

    pub fn get_selected_snapshot(&self) -> Option<&Snapshot> {
        self.table_state.selected().and_then(|i| self.snapshots.get(i))
    }

    pub fn delete_selected_snapshot(&mut self) {
        // Determine which snapshots to delete
        let to_delete: Vec<u32> = if !self.selected_indices.is_empty() {
            // Delete all selected snapshots
            self.selected_indices.iter()
                .filter_map(|&idx| self.snapshots.get(idx))
                .map(|snapshot| snapshot.number)
                .collect()
        } else if let Some(idx) = self.table_state.selected() {
            // Delete single currently highlighted snapshot
            if let Some(snapshot) = self.snapshots.get(idx) {
                vec![snapshot.number]
            } else {
                vec![]
            }
        } else {
            vec![]
        };

        // Perform deletions
        let mut success_count = 0;
        let mut error_count = 0;
        
        for number in to_delete {
            match data::delete_snapshot(number) {
                Ok(_) => success_count += 1,
                Err(_) => error_count += 1,
            }
        }

        // Update message
        if success_count > 0 {
            self.message = if success_count == 1 {
                format!("Deleted 1 snapshot")
            } else {
                format!("Deleted {} snapshots", success_count)
            };
            if error_count > 0 {
                self.message.push_str(&format!(" ({} failed)", error_count));
            }
        } else if error_count > 0 {
            self.message = format!("Failed to delete {} snapshot(s)", error_count);
        }

        // Clear selections and refresh
        self.clear_selections();
        self.refresh_snapshots();
    }

    pub fn apply_selected_snapshot(&mut self) {
        if let Some(snap) = self.get_selected_snapshot() {
            let number = snap.number;
            self.message = format!("Applying snapshot {} (Rollback)...", number);
            match data::rollback_snapshot(number) {
                Ok(_) => {
                    self.message = format!("Snapshot {} applied. Reboot to take effect.", number);
                }
                Err(e) => {
                    self.message = format!("Error applying snapshot: {}", e);
                }
            }
        }
    }
    
    pub fn get_status_selected_snapshot(&mut self) {
         if let Some(snap) = self.get_selected_snapshot().cloned() {
            self.message = format!("Fetching status for {}...", snap.number);
            match data::get_snapshot_status(&snap) {
                Ok(status) => {
                    self.status_text = status;
                    self.message = format!("Status loaded for snapshot {}.", snap.number);
                    self.status_scroll = 0; // Reset scroll
                }
                Err(e) => {
                    self.message = format!("Error getting status: {}", e);
                    self.status_text.clear();
                }
            }
        }
    }

    pub fn on_tick(&mut self) {
        if self.loading {
            self.spinner_state = (self.spinner_state + 1) % self.spinner_frames.len();
        }
    }

    pub fn scroll_details(&mut self, up: bool) {
        if up {
            if self.details_scroll > 0 {
                self.details_scroll -= 1;
            }
        } else {
            self.details_scroll += 1;
        }
    }

    pub fn scroll_status(&mut self, up: bool) {
        if up {
            if self.status_scroll > 0 {
                self.status_scroll -= 1;
            }
        } else {
            self.status_scroll += 1;
        }
    }

    pub fn set_sort_key(&mut self, key: SortKey) {
        // Toggle ascending/descending if same key
        if matches!((&self.current_sort_key, &key),
            (SortKey::Number, SortKey::Number) |
            (SortKey::Type, SortKey::Type) |
            (SortKey::Date, SortKey::Date) |
            (SortKey::User, SortKey::User) |
            (SortKey::UsedSpace, SortKey::UsedSpace))
        {
            self.sort_ascending = !self.sort_ascending;
        } else {
            self.current_sort_key = key;
            self.sort_ascending = true;
        }
        self.sort_snapshots();
    }

    pub fn sort_snapshots(&mut self) {
        match self.current_sort_key {
            SortKey::Number => {
                self.snapshots.sort_by_key(|s| s.number);
            }
            SortKey::Type => {
                self.snapshots.sort_by(|a, b| a.snapshot_type.cmp(&b.snapshot_type));
            }
            SortKey::Date => {
                self.snapshots.sort_by(|a, b| a.date.cmp(&b.date));
            }
            SortKey::User => {
                self.snapshots.sort_by(|a, b| a.user.cmp(&b.user));
            }
            SortKey::UsedSpace => {
                self.snapshots.sort_by_key(|s| s.used_space.unwrap_or(0));
            }
        }
        if !self.sort_ascending {
            self.snapshots.reverse();
        }
    }

    pub fn get_sort_indicator(&self, key: SortKey) -> &'static str {
        let is_active = matches!((&self.current_sort_key, &key),
            (SortKey::Number, SortKey::Number) |
            (SortKey::Type, SortKey::Type) |
            (SortKey::Date, SortKey::Date) |
            (SortKey::User, SortKey::User) |
            (SortKey::UsedSpace, SortKey::UsedSpace));
        
        if is_active {
            if self.sort_ascending { " ↑" } else { " ↓" }
        } else {
            ""
        }
    }
    
    pub fn toggle_selection(&mut self) {
        if let Some(idx) = self.table_state.selected() {
            if self.selected_indices.contains(&idx) {
                self.selected_indices.remove(&idx);
            } else {
                self.selected_indices.insert(idx);
            }
        }
    }
    
    pub fn clear_selections(&mut self) {
        self.selected_indices.clear();
    }
    
    pub fn get_selected_count(&self) -> usize {
        self.selected_indices.len()
    }
}

// Helper function for human-readable sizes
pub fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{}B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1}K", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1}M", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1}G", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}
