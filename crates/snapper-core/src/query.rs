use crate::model::{Snapshot, SnapshotKey};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortKey {
    Number,
    Config,
    Type,
    Date,
    User,
    UsedSpace,
    Cleanup,
    Description,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TypeFilter {
    #[default]
    All,
    Single,
    PrePost,
    Important,
    Pinned,
}

impl TypeFilter {
    pub fn label(&self) -> &'static str {
        match self {
            Self::All => "All types",
            Self::Single => "Single",
            Self::PrePost => "Pre / Post",
            Self::Important => "Important",
            Self::Pinned => "Pinned",
        }
    }

    pub fn all_variants() -> &'static [Self] {
        &[
            Self::All,
            Self::Single,
            Self::PrePost,
            Self::Important,
            Self::Pinned,
        ]
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SnapshotFilter {
    pub query: String,
    pub config: Option<String>,
    pub type_filter: TypeFilter,
}

/// Filter snapshots and return matching indices into `snapshots`.
pub fn filter_snapshots(snapshots: &[Snapshot], filter: &SnapshotFilter) -> Vec<usize> {
    let query_lower = filter.query.trim().to_lowercase();
    snapshots
        .iter()
        .enumerate()
        .filter(|(_, snap)| matches_filter(snap, filter, &query_lower))
        .map(|(idx, _)| idx)
        .collect()
}

pub fn matches_filter(snap: &Snapshot, filter: &SnapshotFilter, query_lower: &str) -> bool {
    if let Some(cfg) = &filter.config
        && &snap.config != cfg
    {
        return false;
    }

    match filter.type_filter {
        TypeFilter::All => {}
        TypeFilter::Single => {
            if snap.snapshot_type != "single" {
                return false;
            }
        }
        TypeFilter::PrePost => {
            if !snap.is_pre() && !snap.is_post() {
                return false;
            }
        }
        TypeFilter::Important => {
            if !snap.is_important() {
                return false;
            }
        }
        TypeFilter::Pinned => {
            if !snap.is_pinned() {
                return false;
            }
        }
    }

    if query_lower.is_empty() {
        return true;
    }

    contains_case_insensitive(&snap.description, query_lower)
        || contains_case_insensitive(&snap.snapshot_type, query_lower)
        || contains_case_insensitive(&snap.user, query_lower)
        || contains_case_insensitive(&snap.config, query_lower)
        || snap
            .cleanup
            .as_deref()
            .is_some_and(|c| contains_case_insensitive(c, query_lower))
        || snap.userdata.as_ref().is_some_and(|map| {
            map.iter().any(|(k, v)| {
                contains_case_insensitive(k, query_lower)
                    || contains_case_insensitive(v, query_lower)
            })
        })
        || matches_number(snap.number, query_lower)
}

pub fn sort_snapshots(snapshots: &mut [Snapshot], key: SortKey, ascending: bool) {
    match key {
        SortKey::Number => {
            snapshots.sort_by(|a, b| a.number.cmp(&b.number).then_with(|| a.config.cmp(&b.config)));
        }
        SortKey::Config => {
            snapshots.sort_by(|a, b| a.config.cmp(&b.config).then_with(|| a.number.cmp(&b.number)));
        }
        SortKey::Type => {
            snapshots.sort_by(|a, b| {
                a.snapshot_type
                    .cmp(&b.snapshot_type)
                    .then_with(|| a.number.cmp(&b.number))
            });
        }
        SortKey::Date => {
            snapshots.sort_by(|a, b| a.date.cmp(&b.date).then_with(|| a.number.cmp(&b.number)));
        }
        SortKey::User => {
            snapshots.sort_by(|a, b| a.user.cmp(&b.user).then_with(|| a.number.cmp(&b.number)));
        }
        SortKey::UsedSpace => {
            snapshots.sort_by(|a, b| {
                a.used_space
                    .unwrap_or(0)
                    .cmp(&b.used_space.unwrap_or(0))
                    .then_with(|| a.number.cmp(&b.number))
            });
        }
        SortKey::Cleanup => {
            snapshots.sort_by(|a, b| {
                a.cleanup
                    .as_deref()
                    .unwrap_or("")
                    .cmp(b.cleanup.as_deref().unwrap_or(""))
                    .then_with(|| a.number.cmp(&b.number))
            });
        }
        SortKey::Description => {
            snapshots.sort_by(|a, b| {
                a.description
                    .to_lowercase()
                    .cmp(&b.description.to_lowercase())
                    .then_with(|| a.number.cmp(&b.number))
            });
        }
    }
    if !ascending {
        snapshots.reverse();
    }
}

/// Resolve a safe, existing `(from, to)` snapshot range for inspecting `snap`.
/// Avoids the naive `number - 1` assumption when intermediate snapshots were pruned.
pub fn resolve_default_compare_range(snap: &Snapshot, all: &[Snapshot]) -> Option<(u32, u32)> {
    let same_config: Vec<&Snapshot> = all.iter().filter(|s| s.config == snap.config).collect();
    let has_number = |num: u32| same_config.iter().any(|s| s.number == num);

    if snap.is_post()
        && let Some(pre) = snap.pre_number
        && has_number(pre)
    {
        return Some((pre, snap.number));
    }

    if snap.is_pre()
        && let Some(post) = snap.post_number
        && has_number(post)
    {
        return Some((snap.number, post));
    }

    if snap.number == 0 {
        let highest = same_config
            .iter()
            .map(|s| s.number)
            .filter(|&n| n > 0)
            .max()?;
        return Some((highest, 0));
    }

    if let Some(prev) = same_config
        .iter()
        .map(|s| s.number)
        .filter(|&n| n > 0 && n < snap.number)
        .max()
    {
        return Some((prev, snap.number));
    }

    if has_number(0) {
        return Some((snap.number, 0));
    }

    None
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConfigStorageStats {
    pub config: String,
    pub snapshot_count: usize,
    pub total_bytes: u64,
    pub pinned_count: usize,
    pub important_count: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StorageSummary {
    pub total_snapshots: usize,
    pub total_used_bytes: u64,
    pub pinned_count: usize,
    pub important_count: usize,
    pub largest_snapshot: Option<SnapshotKey>,
    pub largest_snapshot_bytes: u64,
    pub per_config: Vec<ConfigStorageStats>,
}

pub fn compute_storage_summary(snapshots: &[Snapshot]) -> StorageSummary {
    let mut summary = StorageSummary {
        total_snapshots: snapshots.len(),
        ..Default::default()
    };

    let mut by_config: BTreeMap<String, ConfigStorageStats> = BTreeMap::new();

    for snap in snapshots {
        let bytes = snap.used_space.unwrap_or(0);
        summary.total_used_bytes = summary.total_used_bytes.saturating_add(bytes);
        if snap.is_pinned() {
            summary.pinned_count += 1;
        }
        if snap.is_important() {
            summary.important_count += 1;
        }
        if snap.used_space.is_some()
            && (summary.largest_snapshot.is_none() || bytes > summary.largest_snapshot_bytes)
        {
            summary.largest_snapshot = Some(snap.key());
            summary.largest_snapshot_bytes = bytes;
        }

        let entry = by_config
            .entry(snap.config.clone())
            .or_insert_with(|| ConfigStorageStats {
                config: snap.config.clone(),
                ..Default::default()
            });
        entry.snapshot_count += 1;
        entry.total_bytes = entry.total_bytes.saturating_add(bytes);
        if snap.is_pinned() {
            entry.pinned_count += 1;
        }
        if snap.is_important() {
            entry.important_count += 1;
        }
    }

    summary.per_config = by_config.into_values().collect();
    summary
}

/// Case-insensitive substring match with a zero-allocation ASCII fast path.
pub fn contains_case_insensitive(haystack: &str, needle_lower: &str) -> bool {
    if needle_lower.is_empty() {
        return true;
    }
    if haystack.is_ascii() && needle_lower.is_ascii() {
        let h = haystack.as_bytes();
        let n = needle_lower.as_bytes();
        if n.len() > h.len() {
            return false;
        }
        return h.windows(n.len()).any(|w| w.eq_ignore_ascii_case(n));
    }
    haystack.to_lowercase().contains(needle_lower)
}

/// Check if a snapshot number's decimal representation contains `query` without allocating.
pub fn matches_number(mut num: u32, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    if !query.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    let mut buf = [b'0'; 10];
    let mut pos = buf.len();
    if num == 0 {
        pos -= 1;
        buf[pos] = b'0';
    } else {
        while num > 0 {
            pos -= 1;
            buf[pos] = b'0' + (num % 10) as u8;
            num /= 10;
        }
    }
    let num_str = std::str::from_utf8(&buf[pos..]).unwrap_or("");
    num_str.contains(query)
}

#[cfg(test)]
mod tests {
    use super::*;

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
            cleanup: Some("number".to_string()),
            description: desc.to_string(),
            userdata: None,
            used_space: Some(number as u64 * 1024),
            default: false,
            active: false,
        }
    }

    #[test]
    fn test_resolve_compare_range_handles_gaps_and_pairs() {
        let mut s0 = make_snap("root", 0, "current");
        s0.cleanup = None;
        let s10 = make_snap("root", 10, "base");
        let mut s15 = make_snap("root", 15, "pre");
        s15.snapshot_type = "pre".to_string();
        s15.post_number = Some(18);
        let mut s18 = make_snap("root", 18, "post");
        s18.snapshot_type = "post".to_string();
        s18.pre_number = Some(15);
        let s25 = make_snap("root", 25, "later");
        let other = make_snap("home", 20, "home snap");

        let all = vec![s0.clone(), s10.clone(), s15.clone(), s18.clone(), s25.clone(), other];

        assert_eq!(resolve_default_compare_range(&s18, &all), Some((15, 18)));
        assert_eq!(resolve_default_compare_range(&s15, &all), Some((15, 18)));
        assert_eq!(resolve_default_compare_range(&s25, &all), Some((18, 25)));
        assert_eq!(resolve_default_compare_range(&s0, &all), Some((25, 0)));
        assert_eq!(resolve_default_compare_range(&s10, &all), Some((10, 0)));
    }

    #[test]
    fn test_filter_and_summary() {
        let mut s1 = make_snap("root", 1, "pacman upgrade");
        s1.userdata = Some(BTreeMap::from([("important".into(), "yes".into())]));
        let mut s2 = make_snap("home", 2, "manual checkpoint");
        s2.cleanup = None;

        let snaps = vec![s1, s2];
        let f = SnapshotFilter {
            query: "important".into(),
            config: None,
            type_filter: TypeFilter::Important,
        };
        assert_eq!(filter_snapshots(&snaps, &f), vec![0]);

        let summary = compute_storage_summary(&snaps);
        assert_eq!(summary.total_snapshots, 2);
        assert_eq!(summary.important_count, 1);
        assert_eq!(summary.pinned_count, 1);
        assert_eq!(summary.largest_snapshot, Some(SnapshotKey::new("home", 2)));
    }
}
