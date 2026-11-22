use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use gpui_kit::{App, Context, EventEmitter, Task};
use snapper_core::backend::{SnapperBackend, is_authentication_required};
use snapper_core::model::{
    CleanupAlgorithm, ConfigInfo, ConfigSetting, CreateSnapshotRequest, DiffOutput, FileChange,
    FileTreeNode, ModifySnapshotRequest, Snapshot, SnapshotKey,
};
use snapper_core::parser::build_file_tree;
use snapper_core::query::{
    SnapshotFilter, SortKey, TypeFilter, filter_snapshots, resolve_default_compare_range,
    sort_snapshots,
};

pub const MAX_DIFF_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveView {
    Snapshots,
    Analytics,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectorTab {
    Overview,
    Changes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastLevel {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Debug, Clone)]
pub enum WorkspaceEvent {
    SnapshotsChanged,
    SelectionChanged,
    StatusChanged,
    DiffChanged,
    ConfigSettingsChanged,
    AuthenticationRequired {
        message: String,
    },
    Toast {
        level: ToastLevel,
        title: String,
        message: String,
    },
}

pub fn is_binary_diff(patch: &str) -> bool {
    if patch.as_bytes().contains(&0) {
        return true;
    }
    patch.lines().any(|line| {
        (line.starts_with("Binary files ") && line.ends_with(" differ"))
            || line == "GIT binary patch"
    })
}

pub struct WorkspaceModel {
    backend: Arc<dyn SnapperBackend>,
    configs: Vec<ConfigInfo>,
    snapshots: Vec<Snapshot>,
    filtered_indices: Vec<usize>,
    filter: SnapshotFilter,
    sort_key: SortKey,
    sort_asc: bool,
    active_view: ActiveView,
    inspector_tab: InspectorTab,

    selected_key: Option<SnapshotKey>,
    checked_keys: BTreeSet<SnapshotKey>,

    loading_snapshots: bool,
    hydrating_space: bool,
    auth_required: bool,
    busy_label: Option<String>,
    last_error: Option<String>,

    compare_range: Option<(String, u32, u32)>,
    compare_pinned: bool,
    status_loading: bool,
    status_error: Option<String>,
    file_changes: Vec<FileChange>,
    file_tree: Vec<FileTreeNode>,
    selected_file: Option<String>,
    checked_files: BTreeSet<String>,

    diff_loading: bool,
    diff_error: Option<String>,
    diff_output: Option<DiffOutput>,
    diff_is_binary: bool,
    diff_for_file: Option<String>,

    settings_config: Option<String>,
    settings_loading: bool,
    settings_error: Option<String>,
    config_settings: Vec<ConfigSetting>,

    refresh_generation: u64,
    status_generation: u64,
    diff_generation: u64,
    settings_generation: u64,

    refresh_task: Option<Task<()>>,
    space_task: Option<Task<()>>,
    status_task: Option<Task<()>>,
    diff_task: Option<Task<()>>,
    mutation_task: Option<Task<()>>,
    settings_task: Option<Task<()>>,
}

impl EventEmitter<WorkspaceEvent> for WorkspaceModel {}

impl WorkspaceModel {
    pub fn new(backend: Arc<dyn SnapperBackend>, _cx: &mut App) -> Self {
        Self {
            backend,
            configs: Vec::new(),
            snapshots: Vec::new(),
            filtered_indices: Vec::new(),
            filter: SnapshotFilter::default(),
            sort_key: SortKey::Number,
            sort_asc: false,
            active_view: ActiveView::Snapshots,
            inspector_tab: InspectorTab::Overview,
            selected_key: None,
            checked_keys: BTreeSet::new(),
            loading_snapshots: false,
            hydrating_space: false,
            auth_required: false,
            busy_label: None,
            last_error: None,
            compare_range: None,
            compare_pinned: false,
            status_loading: false,
            status_error: None,
            file_changes: Vec::new(),
            file_tree: Vec::new(),
            selected_file: None,
            checked_files: BTreeSet::new(),
            diff_loading: false,
            diff_error: None,
            diff_output: None,
            diff_is_binary: false,
            diff_for_file: None,
            settings_config: None,
            settings_loading: false,
            settings_error: None,
            config_settings: Vec::new(),
            refresh_generation: 0,
            status_generation: 0,
            diff_generation: 0,
            settings_generation: 0,
            refresh_task: None,
            space_task: None,
            status_task: None,
            diff_task: None,
            mutation_task: None,
            settings_task: None,
        }
    }

    pub fn backend(&self) -> &Arc<dyn SnapperBackend> {
        &self.backend
    }

    pub fn backend_label(&self) -> String {
        self.backend.backend_label()
    }

    pub fn configs(&self) -> &[ConfigInfo] {
        &self.configs
    }

    pub fn snapshots(&self) -> &[Snapshot] {
        &self.snapshots
    }

    pub fn filtered_indices(&self) -> &[usize] {
        &self.filtered_indices
    }

    pub fn filter(&self) -> &SnapshotFilter {
        &self.filter
    }

    pub fn sort_key(&self) -> SortKey {
        self.sort_key
    }

    pub fn sort_asc(&self) -> bool {
        self.sort_asc
    }

    pub fn active_view(&self) -> ActiveView {
        self.active_view
    }

    pub fn inspector_tab(&self) -> InspectorTab {
        self.inspector_tab
    }

    pub fn selected_key(&self) -> Option<&SnapshotKey> {
        self.selected_key.as_ref()
    }

    pub fn checked_keys(&self) -> &BTreeSet<SnapshotKey> {
        &self.checked_keys
    }

    pub fn loading_snapshots(&self) -> bool {
        self.loading_snapshots
    }

    pub fn hydrating_space(&self) -> bool {
        self.hydrating_space
    }

    pub fn auth_required(&self) -> bool {
        self.auth_required
    }

    pub fn is_authenticated(&self) -> bool {
        self.backend.is_authenticated()
    }

    pub fn supports_password_auth(&self) -> bool {
        self.backend.supports_password_auth()
    }

    pub fn busy_label(&self) -> Option<&str> {
        self.busy_label.as_deref()
    }

    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    pub fn compare_range(&self) -> Option<(&str, u32, u32)> {
        self.compare_range
            .as_ref()
            .map(|(cfg, from, to)| (cfg.as_str(), *from, *to))
    }

    pub fn compare_pinned(&self) -> bool {
        self.compare_pinned
    }

    pub fn status_loading(&self) -> bool {
        self.status_loading
    }

    pub fn status_error(&self) -> Option<&str> {
        self.status_error.as_deref()
    }

    pub fn file_changes(&self) -> &[FileChange] {
        &self.file_changes
    }

    pub fn file_tree(&self) -> &[FileTreeNode] {
        &self.file_tree
    }

    pub fn selected_file(&self) -> Option<&str> {
        self.selected_file.as_deref()
    }

    pub fn checked_files(&self) -> &BTreeSet<String> {
        &self.checked_files
    }

    pub fn diff_loading(&self) -> bool {
        self.diff_loading
    }

    pub fn diff_error(&self) -> Option<&str> {
        self.diff_error.as_deref()
    }

    pub fn diff_output(&self) -> Option<&DiffOutput> {
        self.diff_output.as_ref()
    }

    pub fn diff_is_binary(&self) -> bool {
        self.diff_is_binary
    }

    pub fn diff_for_file(&self) -> Option<&str> {
        self.diff_for_file.as_deref()
    }

    pub fn settings_config(&self) -> Option<&str> {
        self.settings_config.as_deref()
    }

    pub fn settings_loading(&self) -> bool {
        self.settings_loading
    }

    pub fn settings_error(&self) -> Option<&str> {
        self.settings_error.as_deref()
    }

    pub fn config_settings(&self) -> &[ConfigSetting] {
        &self.config_settings
    }

    pub fn filtered_snapshot(&self, filtered_row: usize) -> Option<&Snapshot> {
        let idx = *self.filtered_indices.get(filtered_row)?;
        self.snapshots.get(idx)
    }

    pub fn selected_filtered_row(&self) -> Option<usize> {
        let selected = self.selected_key.as_ref()?;
        self.filtered_indices
            .iter()
            .position(|&idx| self.snapshots.get(idx).is_some_and(|s| &s.key() == selected))
    }

    pub fn selected_snapshot(&self) -> Option<&Snapshot> {
        let key = self.selected_key.as_ref()?;
        self.snapshots.iter().find(|s| &s.key() == key)
    }

    pub fn snapshot_by_key(&self, key: &SnapshotKey) -> Option<&Snapshot> {
        self.snapshots.iter().find(|s| &s.key() == key)
    }

    pub fn checked_snapshots(&self) -> Vec<&Snapshot> {
        self.snapshots
            .iter()
            .filter(|s| self.checked_keys.contains(&s.key()))
            .collect()
    }

    pub fn config_snapshot_count(&self, config: &str) -> usize {
        self.snapshots.iter().filter(|s| s.config == config).count()
    }

    pub fn same_config_checked_pair(&self) -> Option<(String, u32, u32)> {
        if self.checked_keys.len() != 2 {
            return None;
        }
        let mut iter = self.checked_keys.iter();
        let a = iter.next()?;
        let b = iter.next()?;
        if a.config != b.config {
            return None;
        }
        let (from, to) = if a.number <= b.number {
            (a.number, b.number)
        } else {
            (b.number, a.number)
        };
        Some((a.config.clone(), from, to))
    }

    fn handle_backend_error(
        &mut self,
        toast_title: &str,
        err: &anyhow::Error,
        emit_toast: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        if is_authentication_required(err) {
            let message = err.to_string();
            self.auth_required = true;
            self.last_error = Some(message.clone());
            cx.emit(WorkspaceEvent::AuthenticationRequired { message });
            return true;
        }
        if emit_toast {
            cx.emit(WorkspaceEvent::Toast {
                level: ToastLevel::Error,
                title: toast_title.to_string(),
                message: err.to_string(),
            });
        }
        false
    }

    pub fn on_authenticated(&mut self, cx: &mut Context<Self>) {
        self.auth_required = false;
        self.last_error = None;
        cx.emit(WorkspaceEvent::Toast {
            level: ToastLevel::Success,
            title: "Authenticated".to_string(),
            message: "Administrator access enabled for this session.".to_string(),
        });
        self.refresh(cx);
    }

    pub fn lock_session(&mut self, cx: &mut Context<Self>) {
        self.backend.clear_authentication();
        self.auth_required = true;
        cx.emit(WorkspaceEvent::SnapshotsChanged);
        cx.emit(WorkspaceEvent::Toast {
            level: ToastLevel::Info,
            title: "Session locked".to_string(),
            message: "Cleared cached administrator credentials from memory.".to_string(),
        });
        cx.notify();
    }

    pub fn request_authentication(&mut self, cx: &mut Context<Self>) {
        let message = self.last_error.clone().unwrap_or_else(|| {
            "Administrator privileges are required to access Snapper snapshots.".to_string()
        });
        cx.emit(WorkspaceEvent::AuthenticationRequired { message });
    }

    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        self.refresh_generation = self.refresh_generation.wrapping_add(1);
        let generation = self.refresh_generation;
        self.loading_snapshots = true;
        self.hydrating_space = false;
        self.last_error = None;
        self.space_task = None;
        cx.emit(WorkspaceEvent::SnapshotsChanged);
        cx.notify();

        let backend = self.backend.clone();
        let single_phase = backend.prefers_single_phase_list();

        self.refresh_task = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let configs = backend.list_configs()?;
                    let snapshots = backend.list_snapshots(None, single_phase)?;
                    Ok::<_, anyhow::Error>((configs, snapshots))
                })
                .await;

            let _ = this.update(cx, |this, cx| {
                if this.refresh_generation != generation {
                    return;
                }
                this.loading_snapshots = false;
                match result {
                    Ok((configs, mut snapshots)) => {
                        this.auth_required = false;
                        this.configs = configs;
                        if this.settings_config.is_none() {
                            this.settings_config = this
                                .configs
                                .first()
                                .map(|c| c.name.clone());
                        }
                        sort_snapshots(&mut snapshots, this.sort_key, this.sort_asc);
                        this.snapshots = snapshots;
                        this.prune_stale_keys();
                        this.recompute_filtered_inner();
                        this.ensure_selection_visible();
                        cx.emit(WorkspaceEvent::SnapshotsChanged);
                        cx.emit(WorkspaceEvent::SelectionChanged);
                        if this.inspector_tab == InspectorTab::Changes && !this.compare_pinned {
                            this.load_status_for_current(cx);
                        }
                        if !single_phase {
                            this.start_space_hydration(generation, cx);
                        }
                    }
                    Err(err) => {
                        let msg = err.to_string();
                        this.last_error = Some(msg);
                        let _ = this.handle_backend_error("Failed to load snapshots", &err, true, cx);
                        cx.emit(WorkspaceEvent::SnapshotsChanged);
                    }
                }
                cx.notify();
            });
        }));
    }

    fn start_space_hydration(&mut self, generation: u64, cx: &mut Context<Self>) {
        self.hydrating_space = true;
        let backend = self.backend.clone();
        self.space_task = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { backend.list_snapshots(None, true) })
                .await;

            let _ = this.update(cx, |this, cx| {
                if this.refresh_generation != generation {
                    return;
                }
                this.hydrating_space = false;
                match result {
                    Ok(detailed) => {
                        let space_map: HashMap<SnapshotKey, Option<u64>> = detailed
                            .into_iter()
                            .map(|s| (s.key(), s.used_space))
                            .collect();
                        for snap in &mut this.snapshots {
                            if let Some(&space) = space_map.get(&snap.key()) {
                                snap.used_space = space;
                            }
                        }
                        if this.sort_key == SortKey::UsedSpace {
                            sort_snapshots(&mut this.snapshots, this.sort_key, this.sort_asc);
                            this.recompute_filtered_inner();
                        }
                        cx.emit(WorkspaceEvent::SnapshotsChanged);
                    }
                    Err(err) => {
                        let _ =
                            this.handle_backend_error("Failed to calculate space", &err, false, cx);
                    }
                }
                cx.notify();
            });
        }));
    }

    fn prune_stale_keys(&mut self) {
        let valid: BTreeSet<SnapshotKey> = self.snapshots.iter().map(|s| s.key()).collect();
        self.checked_keys.retain(|k| valid.contains(k));
        if let Some(selected) = &self.selected_key
            && !valid.contains(selected)
        {
            self.selected_key = None;
        }
    }

    fn ensure_selection_visible(&mut self) {
        let is_visible = self.selected_filtered_row().is_some();
        if !is_visible {
            self.selected_key = self
                .filtered_indices
                .first()
                .and_then(|&idx| self.snapshots.get(idx))
                .map(|s| s.key());
        }
    }

    fn recompute_filtered_inner(&mut self) {
        self.filtered_indices = filter_snapshots(&self.snapshots, &self.filter);
    }

    pub fn set_active_view(&mut self, view: ActiveView, cx: &mut Context<Self>) {
        if self.active_view == view {
            return;
        }
        self.active_view = view;
        if view == ActiveView::Settings
            && self.config_settings.is_empty()
            && !self.settings_loading
            && let Some(cfg) = self
                .filter
                .config
                .clone()
                .or_else(|| self.settings_config.clone())
                .or_else(|| self.configs.first().map(|c| c.name.clone()))
        {
            self.load_config_settings(cfg, cx);
        }
        cx.notify();
    }

    pub fn set_config_filter(&mut self, config: Option<String>, cx: &mut Context<Self>) {
        if self.filter.config == config {
            return;
        }
        if let Some(cfg) = &config {
            self.settings_config = Some(cfg.clone());
        }
        self.filter.config = config;
        self.recompute_filtered_inner();
        self.ensure_selection_visible();
        cx.emit(WorkspaceEvent::SnapshotsChanged);
        cx.emit(WorkspaceEvent::SelectionChanged);
        if self.inspector_tab == InspectorTab::Changes && !self.compare_pinned {
            self.load_status_for_current(cx);
        }
        cx.notify();
    }

    pub fn set_query(&mut self, query: String, cx: &mut Context<Self>) {
        if self.filter.query == query {
            return;
        }
        self.filter.query = query;
        self.recompute_filtered_inner();
        self.ensure_selection_visible();
        cx.emit(WorkspaceEvent::SnapshotsChanged);
        cx.emit(WorkspaceEvent::SelectionChanged);
        cx.notify();
    }

    pub fn set_type_filter(&mut self, type_filter: TypeFilter, cx: &mut Context<Self>) {
        if self.filter.type_filter == type_filter {
            return;
        }
        self.filter.type_filter = type_filter;
        self.recompute_filtered_inner();
        self.ensure_selection_visible();
        cx.emit(WorkspaceEvent::SnapshotsChanged);
        cx.emit(WorkspaceEvent::SelectionChanged);
        cx.notify();
    }

    pub fn set_sort(&mut self, key: SortKey, asc: bool, cx: &mut Context<Self>) {
        self.sort_key = key;
        self.sort_asc = asc;
        sort_snapshots(&mut self.snapshots, key, asc);
        self.recompute_filtered_inner();
        cx.emit(WorkspaceEvent::SnapshotsChanged);
        cx.notify();
    }

    pub fn select_key(&mut self, key: SnapshotKey, cx: &mut Context<Self>) {
        if self.selected_key.as_ref() == Some(&key) {
            return;
        }
        self.selected_key = Some(key);
        cx.emit(WorkspaceEvent::SelectionChanged);
        if self.inspector_tab == InspectorTab::Changes && !self.compare_pinned {
            self.load_status_for_current(cx);
        }
        cx.notify();
    }

    pub fn select_filtered_row(&mut self, row_ix: usize, cx: &mut Context<Self>) {
        if let Some(key) = self.filtered_snapshot(row_ix).map(|s| s.key()) {
            self.select_key(key, cx);
        }
    }

    pub fn toggle_checked(&mut self, key: SnapshotKey, cx: &mut Context<Self>) {
        if !self.checked_keys.remove(&key) {
            self.checked_keys.insert(key);
        }
        cx.emit(WorkspaceEvent::SnapshotsChanged);
        cx.notify();
    }

    pub fn clear_checked(&mut self, cx: &mut Context<Self>) {
        if self.checked_keys.is_empty() {
            return;
        }
        self.checked_keys.clear();
        cx.emit(WorkspaceEvent::SnapshotsChanged);
        cx.notify();
    }

    pub fn set_inspector_tab(&mut self, tab: InspectorTab, cx: &mut Context<Self>) {
        if self.inspector_tab == tab {
            return;
        }
        self.inspector_tab = tab;
        if tab == InspectorTab::Changes {
            let expected = if self.compare_pinned {
                self.compare_range.clone()
            } else {
                self.default_compare_for_selected()
            };
            if expected.is_some() && expected != self.compare_range || self.file_changes.is_empty()
            {
                self.load_status_for_current(cx);
            }
        }
        cx.notify();
    }

    pub fn set_custom_compare(&mut self, config: String, from: u32, to: u32, cx: &mut Context<Self>) {
        self.compare_pinned = true;
        self.compare_range = Some((config.clone(), from, to));
        self.active_view = ActiveView::Snapshots;
        self.inspector_tab = InspectorTab::Changes;
        self.start_status_load(config, from, to, cx);
        cx.notify();
    }

    pub fn clear_custom_compare(&mut self, cx: &mut Context<Self>) {
        self.compare_pinned = false;
        self.load_status_for_current(cx);
    }

    fn default_compare_for_selected(&self) -> Option<(String, u32, u32)> {
        let selected = self.selected_snapshot()?;
        let (from, to) = resolve_default_compare_range(selected, &self.snapshots)?;
        Some((selected.config.clone(), from, to))
    }

    pub fn load_status_for_current(&mut self, cx: &mut Context<Self>) {
        let range = if self.compare_pinned {
            self.compare_range.clone()
        } else {
            self.default_compare_for_selected()
        };
        let Some((config, from, to)) = range else {
            self.status_generation = self.status_generation.wrapping_add(1);
            self.status_task = None;
            self.diff_task = None;
            self.compare_range = None;
            self.status_loading = false;
            self.status_error = None;
            self.file_changes.clear();
            self.file_tree.clear();
            self.selected_file = None;
            self.checked_files.clear();
            self.clear_diff_state();
            cx.emit(WorkspaceEvent::StatusChanged);
            cx.emit(WorkspaceEvent::DiffChanged);
            cx.notify();
            return;
        };
        self.start_status_load(config, from, to, cx);
    }

    fn start_status_load(&mut self, config: String, from: u32, to: u32, cx: &mut Context<Self>) {
        self.status_generation = self.status_generation.wrapping_add(1);
        let generation = self.status_generation;
        self.compare_range = Some((config.clone(), from, to));
        self.status_loading = true;
        self.status_error = None;
        self.file_changes.clear();
        self.file_tree.clear();
        self.selected_file = None;
        self.checked_files.clear();
        self.clear_diff_state();
        cx.emit(WorkspaceEvent::StatusChanged);
        cx.emit(WorkspaceEvent::DiffChanged);
        cx.notify();

        let backend = self.backend.clone();
        self.status_task = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let changes = backend.snapshot_status(&config, from, to)?;
                    let tree = build_file_tree(&changes);
                    Ok::<_, anyhow::Error>((changes, tree))
                })
                .await;

            let _ = this.update(cx, |this, cx| {
                if this.status_generation != generation {
                    return;
                }
                this.status_loading = false;
                match result {
                    Ok((changes, tree)) => {
                        this.selected_file = changes.first().map(|c| c.path.clone());
                        this.file_changes = changes;
                        this.file_tree = tree;
                        cx.emit(WorkspaceEvent::StatusChanged);
                        if this.selected_file.is_some() {
                            this.load_diff_for_selected_file(cx);
                        } else {
                            cx.emit(WorkspaceEvent::DiffChanged);
                        }
                    }
                    Err(err) => {
                        this.status_error = Some(err.to_string());
                        let _ = this.handle_backend_error("Failed to load status", &err, false, cx);
                        cx.emit(WorkspaceEvent::StatusChanged);
                    }
                }
                cx.notify();
            });
        }));
    }

    fn clear_diff_state(&mut self) {
        self.diff_generation = self.diff_generation.wrapping_add(1);
        self.diff_task = None;
        self.diff_loading = false;
        self.diff_error = None;
        self.diff_output = None;
        self.diff_is_binary = false;
        self.diff_for_file = None;
    }

    pub fn select_file(&mut self, path: String, cx: &mut Context<Self>) {
        if self.selected_file.as_deref() == Some(path.as_str()) && self.diff_output.is_some() {
            return;
        }
        self.selected_file = Some(path);
        cx.emit(WorkspaceEvent::StatusChanged);
        self.load_diff_for_selected_file(cx);
        cx.notify();
    }

    pub fn toggle_checked_file(&mut self, path: String, cx: &mut Context<Self>) {
        if !self.checked_files.remove(&path) {
            self.checked_files.insert(path);
        }
        cx.emit(WorkspaceEvent::StatusChanged);
        cx.notify();
    }

    pub fn set_checked_files_for_paths(
        &mut self,
        paths: impl IntoIterator<Item = String>,
        checked: bool,
        cx: &mut Context<Self>,
    ) {
        if checked {
            self.checked_files.extend(paths);
        } else {
            let remove: BTreeSet<String> = paths.into_iter().collect();
            self.checked_files.retain(|p| !remove.contains(p));
        }
        cx.emit(WorkspaceEvent::StatusChanged);
        cx.notify();
    }

    pub fn load_diff_for_selected_file(&mut self, cx: &mut Context<Self>) {
        let Some((config, from, to)) = self.compare_range.clone() else {
            self.clear_diff_state();
            cx.emit(WorkspaceEvent::DiffChanged);
            return;
        };
        let Some(file_path) = self.selected_file.clone() else {
            self.clear_diff_state();
            cx.emit(WorkspaceEvent::DiffChanged);
            return;
        };

        self.diff_generation = self.diff_generation.wrapping_add(1);
        let generation = self.diff_generation;
        self.diff_loading = true;
        self.diff_error = None;
        self.diff_output = None;
        self.diff_is_binary = false;
        self.diff_for_file = Some(file_path.clone());
        cx.emit(WorkspaceEvent::DiffChanged);
        cx.notify();

        let backend = self.backend.clone();
        self.diff_task = Some(cx.spawn(async move |this, cx| {
            let files = vec![file_path.clone()];
            let result = cx
                .background_executor()
                .spawn(async move {
                    backend.snapshot_diff(&config, from, to, &files, MAX_DIFF_BYTES)
                })
                .await;

            let _ = this.update(cx, |this, cx| {
                if this.diff_generation != generation {
                    return;
                }
                this.diff_loading = false;
                match result {
                    Ok(output) => {
                        this.diff_is_binary = is_binary_diff(&output.patch);
                        this.diff_output = Some(output);
                    }
                    Err(err) => {
                        this.diff_error = Some(err.to_string());
                        let _ = this.handle_backend_error("Failed to load diff", &err, false, cx);
                    }
                }
                cx.emit(WorkspaceEvent::DiffChanged);
                cx.notify();
            });
        }));
    }

    pub fn create_snapshot(&mut self, req: CreateSnapshotRequest, cx: &mut Context<Self>) {
        self.busy_label = Some(format!("Creating snapshot in {}…", req.config));
        cx.notify();
        let backend = self.backend.clone();
        self.mutation_task = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { backend.create_snapshot(&req).map(|num| (req.config, num)) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.busy_label = None;
                match result {
                    Ok((config, num)) => {
                        let label = num
                            .map(|n| format!("#{n}"))
                            .unwrap_or_else(|| "snapshot".to_string());
                        if let Some(n) = num {
                            this.selected_key = Some(SnapshotKey::new(config.clone(), n));
                        }
                        cx.emit(WorkspaceEvent::Toast {
                            level: ToastLevel::Success,
                            title: "Snapshot created".to_string(),
                            message: format!("Created {label} in config '{config}'."),
                        });
                        this.refresh(cx);
                    }
                    Err(err) => {
                        let _ =
                            this.handle_backend_error("Create snapshot failed", &err, true, cx);
                    }
                }
                cx.notify();
            });
        }));
    }

    pub fn modify_snapshot(&mut self, req: ModifySnapshotRequest, cx: &mut Context<Self>) {
        let key = req.key.clone();
        self.busy_label = Some(format!("Updating {}…", key));
        cx.notify();
        let backend = self.backend.clone();
        self.mutation_task = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { backend.modify_snapshot(&req) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.busy_label = None;
                match result {
                    Ok(()) => {
                        cx.emit(WorkspaceEvent::Toast {
                            level: ToastLevel::Success,
                            title: "Snapshot updated".to_string(),
                            message: format!("Updated {}.", key),
                        });
                        this.refresh(cx);
                    }
                    Err(err) => {
                        let _ = this.handle_backend_error("Update failed", &err, true, cx);
                    }
                }
                cx.notify();
            });
        }));
    }

    pub fn toggle_pin_selected(&mut self, cx: &mut Context<Self>) {
        let Some(snap) = self.selected_snapshot().cloned() else {
            return;
        };
        if snap.number == 0 {
            cx.emit(WorkspaceEvent::Toast {
                level: ToastLevel::Warning,
                title: "Cannot pin #0".to_string(),
                message: "Snapshot #0 is always the live subvolume.".to_string(),
            });
            return;
        }
        let currently_pinned = snap.is_pinned();
        let next_cleanup = if currently_pinned {
            Some(CleanupAlgorithm::Number)
        } else {
            None
        };
        let req = ModifySnapshotRequest {
            key: snap.key(),
            description: Some(snap.description),
            cleanup: Some(next_cleanup),
            userdata: snap.userdata,
        };
        self.modify_snapshot(req, cx);
    }

    pub fn delete_snapshots(&mut self, targets: Vec<SnapshotKey>, cx: &mut Context<Self>) {
        let deletable: Vec<SnapshotKey> = targets
            .into_iter()
            .filter(|k| {
                self.snapshot_by_key(k)
                    .map(|s| !s.is_protected())
                    .unwrap_or(k.number != 0)
            })
            .collect();
        if deletable.is_empty() {
            cx.emit(WorkspaceEvent::Toast {
                level: ToastLevel::Warning,
                title: "Nothing deletable selected".to_string(),
                message: "Snapshot #0, active, and default snapshots are protected.".to_string(),
            });
            return;
        }
        let total = deletable.len();
        self.busy_label = Some(format!("Deleting {total} snapshot(s)…"));
        cx.notify();
        let backend = self.backend.clone();
        self.mutation_task = Some(cx.spawn(async move |this, cx| {
            let report = cx
                .background_executor()
                .spawn(async move { backend.delete_snapshots(&deletable) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.busy_label = None;
                for key in &report.deleted {
                    this.checked_keys.remove(key);
                }
                if report.failed.is_empty() {
                    cx.emit(WorkspaceEvent::Toast {
                        level: ToastLevel::Success,
                        title: "Snapshots deleted".to_string(),
                        message: format!("Deleted {} snapshot(s).", report.deleted.len()),
                    });
                } else {
                    let first_err = &report.failed[0].1;
                    if !this.backend.is_authenticated() && this.backend.supports_password_auth() {
                        this.auth_required = true;
                        this.last_error = Some(first_err.clone());
                        cx.emit(WorkspaceEvent::AuthenticationRequired {
                            message: first_err.clone(),
                        });
                    } else {
                        cx.emit(WorkspaceEvent::Toast {
                            level: ToastLevel::Warning,
                            title: "Partial deletion".to_string(),
                            message: format!(
                                "Deleted {}, failed {}: {first_err}",
                                report.deleted.len(),
                                report.failed.len()
                            ),
                        });
                    }
                }
                this.refresh(cx);
                cx.notify();
            });
        }));
    }

    pub fn rollback_snapshot(&mut self, key: SnapshotKey, cx: &mut Context<Self>) {
        self.busy_label = Some(format!("Rolling back to {}…", key));
        cx.notify();
        let backend = self.backend.clone();
        self.mutation_task = Some(cx.spawn(async move |this, cx| {
            let key_clone = key.clone();
            let result = cx
                .background_executor()
                .spawn(async move { backend.rollback_snapshot(&key_clone) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.busy_label = None;
                match result {
                    Ok(()) => {
                        cx.emit(WorkspaceEvent::Toast {
                            level: ToastLevel::Success,
                            title: "Rollback complete".to_string(),
                            message: format!(
                                "Rolled back {} — reboot into the updated default subvolume.",
                                key
                            ),
                        });
                        this.refresh(cx);
                    }
                    Err(err) => {
                        let _ = this.handle_backend_error("Rollback failed", &err, true, cx);
                    }
                }
                cx.notify();
            });
        }));
    }

    pub fn undo_changes(
        &mut self,
        config: String,
        from: u32,
        to: u32,
        files: Vec<String>,
        cx: &mut Context<Self>,
    ) {
        if files.is_empty() {
            return;
        }
        let count = files.len();
        self.busy_label = Some(format!("Restoring {count} file(s)…"));
        cx.notify();
        let backend = self.backend.clone();
        self.mutation_task = Some(cx.spawn(async move |this, cx| {
            let cfg = config.clone();
            let result = cx
                .background_executor()
                .spawn(async move { backend.undo_changes(&cfg, from, to, &files) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.busy_label = None;
                match result {
                    Ok(summary) => {
                        this.checked_files.clear();
                        let msg = if summary.is_empty() {
                            format!("Restored {count} path(s) in '{config}' ({from}..{to}).")
                        } else {
                            summary
                        };
                        cx.emit(WorkspaceEvent::Toast {
                            level: ToastLevel::Success,
                            title: "Files restored".to_string(),
                            message: msg,
                        });
                        this.load_status_for_current(cx);
                    }
                    Err(err) => {
                        let _ = this.handle_backend_error("Restore failed", &err, true, cx);
                    }
                }
                cx.notify();
            });
        }));
    }

    pub fn load_config_settings(&mut self, config: String, cx: &mut Context<Self>) {
        self.settings_generation = self.settings_generation.wrapping_add(1);
        let generation = self.settings_generation;
        self.settings_config = Some(config.clone());
        self.settings_loading = true;
        self.settings_error = None;
        cx.emit(WorkspaceEvent::ConfigSettingsChanged);
        cx.notify();

        let backend = self.backend.clone();
        self.settings_task = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { backend.get_config(&config) })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.settings_generation != generation {
                    return;
                }
                this.settings_loading = false;
                match result {
                    Ok(settings) => {
                        this.config_settings = settings;
                    }
                    Err(err) => {
                        this.settings_error = Some(err.to_string());
                        let _ = this.handle_backend_error("Failed to load config", &err, false, cx);
                    }
                }
                cx.emit(WorkspaceEvent::ConfigSettingsChanged);
                cx.notify();
            });
        }));
    }

    pub fn save_config_settings(
        &mut self,
        config: String,
        settings: Vec<ConfigSetting>,
        cx: &mut Context<Self>,
    ) {
        if settings.is_empty() {
            return;
        }
        self.busy_label = Some(format!("Saving config '{config}'…"));
        cx.notify();
        let backend = self.backend.clone();
        self.mutation_task = Some(cx.spawn(async move |this, cx| {
            let cfg = config.clone();
            let count = settings.len();
            let result = cx
                .background_executor()
                .spawn(async move { backend.set_config(&cfg, &settings) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.busy_label = None;
                match result {
                    Ok(()) => {
                        cx.emit(WorkspaceEvent::Toast {
                            level: ToastLevel::Success,
                            title: "Config saved".to_string(),
                            message: format!("Updated {count} setting(s) for '{config}'."),
                        });
                        this.load_config_settings(config, cx);
                    }
                    Err(err) => {
                        let _ = this.handle_backend_error("Save config failed", &err, true, cx);
                    }
                }
                cx.notify();
            });
        }));
    }

    pub fn run_cleanup(
        &mut self,
        config: String,
        algorithm: CleanupAlgorithm,
        cx: &mut Context<Self>,
    ) {
        let alg_str = algorithm.as_cli_str().to_string();
        self.busy_label = Some(format!("Running {alg_str} cleanup on '{config}'…"));
        cx.notify();
        let backend = self.backend.clone();
        self.mutation_task = Some(cx.spawn(async move |this, cx| {
            let cfg = config.clone();
            let result = cx
                .background_executor()
                .spawn(async move { backend.run_cleanup(&cfg, algorithm) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.busy_label = None;
                match result {
                    Ok(()) => {
                        cx.emit(WorkspaceEvent::Toast {
                            level: ToastLevel::Success,
                            title: "Cleanup completed".to_string(),
                            message: format!("Ran '{alg_str}' cleanup on '{config}'."),
                        });
                        this.refresh(cx);
                    }
                    Err(err) => {
                        let _ = this.handle_backend_error("Cleanup failed", &err, true, cx);
                    }
                }
                cx.notify();
            });
        }));
    }
}
