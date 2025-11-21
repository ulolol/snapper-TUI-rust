use crate::data::{self, Snapshot};
use ratatui::widgets::TableState;
use std::sync::mpsc::{self, Receiver, Sender};
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
    pub sort_key: SortKey,
    pub sort_reverse: bool,
    pub loading: bool,
    pub message: String,
    // New fields for scrolling and spinner
    pub details_scroll: u16,
    pub status_scroll: u16,
    pub status_text: String,
    pub spinner_state: usize,
    pub spinner_frames: Vec<&'static str>,
    // Threading
    pub rx: Option<Receiver<Result<Vec<Snapshot>, String>>>,
    // TachyonFX
    pub fx_start: Option<std::time::Instant>,
    pub fx: Option<Effect>,
    // Popups
    pub show_delete_popup: bool,
    pub show_apply_popup: bool,
    // Splash Screen
    pub show_splash: bool,
    pub splash_start: Option<std::time::Instant>,
}

impl App {
    pub fn new() -> App {
        App {
            snapshots: Vec::new(),
            table_state: TableState::default(),
            sort_key: SortKey::Number,
            sort_reverse: false,
            loading: true, // Start loading immediately
            message: String::from("Initializing..."),
            details_scroll: 0,
            status_scroll: 0,
            status_text: String::new(),
            spinner_state: 0,
            spinner_frames: vec!["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"],
            rx: None,
            fx_start: None,
            fx: None,
            show_delete_popup: false,
            show_apply_popup: false,
            show_splash: true,
            splash_start: Some(std::time::Instant::now()),
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

    pub fn sort_snapshots(&mut self) {
        self.snapshots.sort_by(|a, b| {
            let ordering = match self.sort_key {
                SortKey::Number => a.number.cmp(&b.number),
                SortKey::Type => a.snapshot_type.cmp(&b.snapshot_type),
                SortKey::Date => a.date.cmp(&b.date),
                SortKey::User => a.user.cmp(&b.user),
                SortKey::UsedSpace => a.used_space.unwrap_or(0).cmp(&b.used_space.unwrap_or(0)),
            };
            if self.sort_reverse {
                ordering.reverse()
            } else {
                ordering
            }
        });
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
        if let Some(snap) = self.get_selected_snapshot() {
            let number = snap.number;
            self.message = format!("Deleting snapshot {}...", number);
            match data::delete_snapshot(number) {
                Ok(_) => {
                    self.message = format!("Snapshot {} deleted.", number);
                    self.refresh_snapshots();
                }
                Err(e) => {
                    self.message = format!("Error deleting snapshot: {}", e);
                }
            }
        }
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
}
