use ratatui::layout::Rect;
use ratatui::widgets::TableState;
use snapper_core::model::{Snapshot, SnapshotKey};
use snapper_core::query::{SnapshotFilter, SortKey, TypeFilter, filter_snapshots, sort_snapshots};
use std::collections::HashSet;
use std::sync::mpsc::Receiver;
use std::time::Instant;
use tachyonfx::Effect;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionButton {
    Create,
    Delete,
    Apply,
    Filter,
    Status,
    Refresh,
    Quit,
}

#[derive(Default, Clone)]
pub struct UiAreas {
    pub table: Rect,
    pub table_header_row: u16,
    pub table_first_data_row: u16,
    pub details: Rect,
    pub status: Rect,
    pub details_max_scroll: u16,
    pub status_max_scroll: u16,
    pub footer_buttons: Vec<(Rect, ActionButton)>,
}

pub enum AsyncResult {
    Snapshots(Vec<Snapshot>),
    Delete {
        success: usize,
        fail: usize,
        first_error: Option<String>,
    },
    Create(String),
    Apply(u32),
    Status {
        request_id: u64,
        text: String,
    },
}

pub struct App {
    pub snapshots: Vec<Snapshot>,
    pub table_state: TableState,
    pub message: String,
    pub loading: bool,
    pub loading_message: String,
    pub status_text: String,
    pub details_scroll: u16,
    pub status_scroll: u16,
    pub spinner_state: usize,
    pub spinner_frames: Vec<&'static str>,
    pub show_delete_popup: bool,
    pub show_create_popup: bool,
    pub create_input: String,
    pub filtering: bool,
    pub filter_input: String,
    pub show_apply_popup: bool,
    pub show_splash: bool,
    pub splash_start: Option<Instant>,
    pub fx: Option<Effect>,
    pub fx_last_tick: Option<Instant>,
    pub current_sort_key: SortKey,
    pub sort_ascending: bool,
    /// Channel for blocking operations (refresh, create, delete, apply, explicit status).
    pub op_rx: Option<Receiver<Result<AsyncResult, String>>>,
    /// Separate channel for background selection status previews so they never clobber `op_rx`.
    pub status_rx: Option<Receiver<Result<AsyncResult, String>>>,
    pub selected_keys: HashSet<SnapshotKey>,
    pub status_request_id: u64,
    pub ui_areas: UiAreas,
}

impl App {
    pub fn new() -> App {
        App {
            snapshots: Vec::new(),
            table_state: TableState::default(),
            message: String::from("⚡ Initializing..."),
            loading: true,
            loading_message: String::from("Loading..."),
            status_text: String::new(),
            details_scroll: 0,
            status_scroll: 0,
            spinner_state: 0,
            spinner_frames: vec!["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"],
            show_delete_popup: false,
            show_create_popup: false,
            create_input: String::new(),
            filtering: false,
            filter_input: String::new(),
            show_apply_popup: false,
            show_splash: true,
            splash_start: Some(Instant::now()),
            fx: None,
            fx_last_tick: None,
            current_sort_key: SortKey::Number,
            sort_ascending: true,
            op_rx: None,
            status_rx: None,
            selected_keys: HashSet::new(),
            status_request_id: 0,
            ui_areas: UiAreas::default(),
        }
    }

    pub fn get_filtered_snapshots(&self) -> Vec<&Snapshot> {
        if self.filter_input.is_empty() {
            return self.snapshots.iter().collect();
        }
        let filter = SnapshotFilter {
            query: self.filter_input.clone(),
            config: None,
            type_filter: TypeFilter::All,
        };
        filter_snapshots(&self.snapshots, &filter)
            .into_iter()
            .filter_map(|idx| self.snapshots.get(idx))
            .collect()
    }

    pub fn clamp_selection(&mut self) {
        let len = self.get_filtered_snapshots().len();
        if len == 0 {
            self.table_state.select(None);
        } else {
            let idx = self.table_state.selected().unwrap_or(0).min(len - 1);
            self.table_state.select(Some(idx));
        }
        self.details_scroll = 0;
    }

    pub fn select_row(&mut self, index: usize) {
        let len = self.get_filtered_snapshots().len();
        if index < len {
            self.table_state.select(Some(index));
            self.details_scroll = 0;
        }
    }

    pub fn next(&mut self) {
        let filtered_len = self.get_filtered_snapshots().len();
        if filtered_len > 0 {
            let i = match self.table_state.selected() {
                Some(i) if i >= filtered_len - 1 => 0,
                Some(i) => i + 1,
                None => 0,
            };
            self.table_state.select(Some(i));
            self.details_scroll = 0;
        }
    }

    pub fn previous(&mut self) {
        let filtered_len = self.get_filtered_snapshots().len();
        if filtered_len > 0 {
            let i = match self.table_state.selected() {
                Some(0) | None => filtered_len - 1,
                Some(i) => i.min(filtered_len - 1).saturating_sub(1),
            };
            if self.table_state.selected().is_none() {
                self.table_state.select(Some(0));
            } else {
                self.table_state.select(Some(i));
            }
            self.details_scroll = 0;
        }
    }

    pub fn page_down(&mut self, step: usize) {
        let filtered_len = self.get_filtered_snapshots().len();
        if filtered_len > 0 {
            let cur = self.table_state.selected().unwrap_or(0);
            let next = (cur + step).min(filtered_len - 1);
            self.table_state.select(Some(next));
            self.details_scroll = 0;
        }
    }

    pub fn page_up(&mut self, step: usize) {
        let filtered_len = self.get_filtered_snapshots().len();
        if filtered_len > 0 {
            let cur = self.table_state.selected().unwrap_or(0);
            let next = cur.saturating_sub(step);
            self.table_state.select(Some(next));
            self.details_scroll = 0;
        }
    }

    pub fn get_selected_snapshot(&self) -> Option<&Snapshot> {
        let idx = self.table_state.selected()?;
        self.get_filtered_snapshots().get(idx).copied()
    }

    pub fn get_targets_for_delete(&self) -> Vec<SnapshotKey> {
        if !self.selected_keys.is_empty() {
            self.snapshots
                .iter()
                .filter(|s| self.selected_keys.contains(&s.key()))
                .map(Snapshot::key)
                .collect()
        } else if let Some(snapshot) = self.get_selected_snapshot() {
            vec![snapshot.key()]
        } else {
            vec![]
        }
    }

    /// Count how many current delete targets are protected (`#0`, `active`, or `default`).
    pub fn count_protected_delete_targets(&self) -> usize {
        let targets = self.get_targets_for_delete();
        targets
            .iter()
            .filter(|k| {
                self.snapshots
                    .iter()
                    .find(|s| &s.key() == *k)
                    .is_some_and(Snapshot::is_protected)
            })
            .count()
    }

    pub fn handle_delete_result(
        &mut self,
        success_count: usize,
        error_count: usize,
        first_error: Option<String>,
    ) {
        if success_count > 0 {
            self.message = if success_count == 1 {
                String::from("🗑️ Deleted 1 snapshot")
            } else {
                format!("🗑️ Deleted {} snapshots", success_count)
            };
            if error_count > 0 {
                self.message.push_str(&format!(" ({} failed ❌)", error_count));
            }
        } else if error_count > 0 {
            self.message = match first_error {
                Some(err) => format!("❌ {}", err),
                None => format!("❌ Failed to delete {} snapshot(s)", error_count),
            };
        }

        self.clear_selections();
    }

    pub fn get_target_for_apply(&self) -> Option<SnapshotKey> {
        self.get_selected_snapshot().map(Snapshot::key)
    }

    pub fn on_tick(&mut self) {
        if self.loading {
            self.spinner_state = (self.spinner_state + 1) % self.spinner_frames.len();
        }
    }

    pub fn scroll_details(&mut self, up: bool) {
        if up {
            self.details_scroll = self.details_scroll.saturating_sub(1);
        } else if self.details_scroll < self.ui_areas.details_max_scroll {
            self.details_scroll = self.details_scroll.saturating_add(1);
        }
    }

    pub fn scroll_status(&mut self, up: bool) {
        if up {
            self.status_scroll = self.status_scroll.saturating_sub(1);
        } else if self.status_scroll < self.ui_areas.status_max_scroll {
            self.status_scroll = self.status_scroll.saturating_add(1);
        }
    }

    pub fn set_sort_key(&mut self, key: SortKey) {
        if self.current_sort_key == key {
            self.sort_ascending = !self.sort_ascending;
        } else {
            self.current_sort_key = key;
            self.sort_ascending = true;
        }
        self.sort_snapshots();
    }

    pub fn sort_snapshots(&mut self) {
        sort_snapshots(&mut self.snapshots, self.current_sort_key, self.sort_ascending);
        self.clamp_selection();
    }

    pub fn get_sort_indicator(&self, key: SortKey) -> &'static str {
        if self.current_sort_key == key {
            if self.sort_ascending { " ↑" } else { " ↓" }
        } else {
            ""
        }
    }

    pub fn toggle_selection(&mut self) {
        if let Some(snap) = self.get_selected_snapshot() {
            let key = snap.key();
            if !self.selected_keys.remove(&key) {
                self.selected_keys.insert(key);
            }
        }
    }

    pub fn clear_selections(&mut self) {
        self.selected_keys.clear();
    }

    pub fn get_selected_count(&self) -> usize {
        self.selected_keys.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snapper_core::format::format_size;

    fn make_snap(config: &str, number: u32, desc: &str) -> Snapshot {
        Snapshot {
            config: config.to_string(),
            subvolume: format!("/.snapshots/{}/snapshot", number),
            number,
            snapshot_type: "single".to_string(),
            pre_number: None,
            post_number: None,
            date: format!("2025-01-{:02} 10:00:00", (number % 28) + 1),
            user: "root".to_string(),
            cleanup: None,
            description: desc.to_string(),
            userdata: None,
            used_space: Some(number as u64 * 1024),
            default: false,
            active: false,
        }
    }

    #[test]
    fn test_selected_snapshot_respects_filter() {
        let mut app = App::new();
        app.snapshots = vec![
            make_snap("root", 1, "first"),
            make_snap("root", 2, "pacman upgrade"),
            make_snap("home", 3, "home backup"),
        ];
        app.filter_input = "pacman".to_string();
        app.table_state.select(Some(0));

        let selected = app.get_selected_snapshot().unwrap();
        assert_eq!(selected.number, 2);
        assert_eq!(selected.description, "pacman upgrade");
    }

    #[test]
    fn test_multi_selection_persists_across_sort_and_filter() {
        let mut app = App::new();
        app.snapshots = vec![
            make_snap("root", 10, "alpha"),
            make_snap("root", 2, "beta"),
            make_snap("home", 2, "gamma"),
        ];
        app.table_state.select(Some(1));
        app.toggle_selection();

        app.sort_snapshots();
        let targets = app.get_targets_for_delete();
        assert_eq!(targets, vec![SnapshotKey::new("root", 2)]);
    }

    #[test]
    fn test_protected_delete_target_counting() {
        let mut app = App::new();
        let mut active_snap = make_snap("root", 10, "active");
        active_snap.active = true;
        let normal = make_snap("root", 11, "normal");
        app.snapshots = vec![active_snap, normal];
        app.table_state.select(Some(0));
        assert_eq!(app.count_protected_delete_targets(), 1);
    }

    #[test]
    fn test_format_size_integration() {
        assert_eq!(format_size(512), "512B");
        assert_eq!(format_size(2048), "2.0K");
        assert_eq!(format_size(5 * 1024 * 1024), "5.0M");
        assert_eq!(format_size(2 * 1024 * 1024 * 1024), "2.0G");
    }
}
