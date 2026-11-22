use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result, anyhow};
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::OwnedValue;

use crate::backend::{ElevationStrategy, SnapperBackend, SnapperCliBackend};
use crate::model::{
    BatchDeleteReport, CleanupAlgorithm, ConfigInfo, ConfigSetting, ContentChange,
    CreateSnapshotRequest, DiffOutput, FileChange, FileStatusFlags, ModifySnapshotRequest,
    Snapshot, SnapshotCreateType, SnapshotKey,
};

const DBUS_SERVICE: &str = "org.opensuse.Snapper";
const DBUS_PATH: &str = "/org/opensuse/Snapper";
const DBUS_INTERFACE: &str = "org.opensuse.Snapper";

type RawConfigRow = (String, String, HashMap<String, String>);
type RawSnapshotRow = (
    u32,
    u16,
    u32,
    i64,
    u32,
    String,
    String,
    HashMap<String, String>,
);
type RawConfigDetail = (String, String, HashMap<String, String>);
type RawFileEntry = (String, u32);

/// Native Snapper D-Bus backend talking to `snapperd` (`org.opensuse.Snapper`) over the system bus.
///
/// Operations not exposed by `org.opensuse.Snapper` (`diff`, `undochange`, `rollback`, `cleanup`)
/// delegate cleanly to an inner `SnapperCliBackend` with `PkexecFallback`.
pub struct SnapperDbusBackend {
    connection: Connection,
    cli_fallback: SnapperCliBackend,
    needs_elevation: AtomicBool,
}

impl SnapperDbusBackend {
    pub fn connect() -> Result<Self> {
        let connection =
            Connection::system().context("failed to connect to D-Bus system bus")?;
        let backend = Self {
            connection,
            cli_fallback: SnapperCliBackend::new(ElevationStrategy::PkexecFallback),
            needs_elevation: AtomicBool::new(false),
        };
        // Probe interface presence so callers can fall back to CLI cleanly if snapperd is unavailable.
        {
            let proxy = backend.proxy()?;
            let _: Vec<RawConfigRow> = proxy
                .call("ListConfigs", &())
                .context("org.opensuse.Snapper.ListConfigs probe failed")?;
        }
        Ok(backend)
    }

    fn proxy(&self) -> Result<Proxy<'_>> {
        Proxy::new(&self.connection, DBUS_SERVICE, DBUS_PATH, DBUS_INTERFACE)
            .context("failed to construct org.opensuse.Snapper D-Bus proxy")
    }

    fn list_snapshots_for_config(
        &self,
        proxy: &Proxy<'_>,
        config: &str,
        subvolume: &str,
        include_used_space: bool,
    ) -> Result<Vec<Snapshot>> {
        let rows: Vec<RawSnapshotRow> = proxy
            .call("ListSnapshots", &(config,))
            .with_context(|| format!("D-Bus ListSnapshots({config}) failed"))?;

        let active_num: Option<u32> = proxy
            .call::<_, _, (bool, u32)>("GetActiveSnapshot", &(config,))
            .ok()
            .and_then(|(ok, num)| ok.then_some(num));
        let default_num: Option<u32> = proxy
            .call::<_, _, (bool, u32)>("GetDefaultSnapshot", &(config,))
            .ok()
            .and_then(|(ok, num)| ok.then_some(num));

        if include_used_space {
            let _ = proxy.call::<_, _, ()>("CalculateUsedSpace", &(config,));
        }

        let mut post_for_pre: HashMap<u32, u32> = HashMap::new();
        for (number, stype, pre_number, _, _, _, _, _) in &rows {
            if *stype == 2 && *pre_number != 0 {
                post_for_pre.insert(*pre_number, *number);
            }
        }

        let uid_names = load_passwd_uids();
        let mut out = Vec::with_capacity(rows.len());

        for (number, stype, pre_raw, date_unix, uid, description, cleanup_raw, userdata_raw) in
            rows
        {
            let snapshot_type = match stype {
                1 => "pre",
                2 => "post",
                _ => "single",
            }
            .to_string();

            let pre_number = if stype == 2 && pre_raw != 0 {
                Some(pre_raw)
            } else {
                None
            };
            let post_number = if stype == 1 {
                post_for_pre.get(&number).copied()
            } else {
                None
            };

            let date = if number == 0 && date_unix <= 0 {
                String::new()
            } else {
                format_unix_timestamp(date_unix)
            };

            let user = if uid == 0 {
                "root".to_string()
            } else {
                uid_names
                    .get(&uid)
                    .cloned()
                    .unwrap_or_else(|| uid.to_string())
            };

            let cleanup = {
                let trimmed = cleanup_raw.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            };

            let userdata = if userdata_raw.is_empty() {
                None
            } else {
                Some(BTreeMap::from_iter(userdata_raw))
            };

            let used_space = if include_used_space && number != 0 {
                proxy
                    .call::<_, _, OwnedValue>("GetUsedSpace", &(config, number))
                    .ok()
                    .and_then(|v| decode_dbus_u64(&v))
            } else {
                None
            };

            out.push(Snapshot {
                config: config.to_string(),
                subvolume: subvolume.to_string(),
                number,
                snapshot_type,
                pre_number,
                post_number,
                date,
                user,
                cleanup,
                description,
                userdata,
                used_space,
                default: default_num == Some(number),
                active: active_num == Some(number),
            });
        }

        Ok(out)
    }
}

impl SnapperBackend for SnapperDbusBackend {
    fn backend_label(&self) -> String {
        if self.needs_elevation.load(Ordering::Relaxed) {
            format!("Snapper D-Bus + {}", self.cli_fallback.backend_label())
        } else {
            "Snapper D-Bus (org.opensuse.Snapper)".to_string()
        }
    }

    fn prefers_single_phase_list(&self) -> bool {
        self.cli_fallback.prefers_single_phase_list()
    }

    fn supports_password_auth(&self) -> bool {
        self.cli_fallback.supports_password_auth()
    }

    fn is_authenticated(&self) -> bool {
        self.cli_fallback.is_authenticated()
    }

    fn authenticate_with_password(&self, password: &str) -> Result<()> {
        self.cli_fallback.authenticate_with_password(password)?;
        self.needs_elevation.store(true, Ordering::Relaxed);
        Ok(())
    }

    fn enable_pkexec_mode(&self) -> Result<()> {
        self.cli_fallback.enable_pkexec_mode()?;
        self.needs_elevation.store(true, Ordering::Relaxed);
        Ok(())
    }

    fn clear_authentication(&self) {
        self.cli_fallback.clear_authentication();
    }

    fn list_configs(&self) -> Result<Vec<ConfigInfo>> {
        if self.needs_elevation.load(Ordering::Relaxed) {
            return self.cli_fallback.list_configs();
        }
        let proxy = self.proxy()?;
        let rows: Vec<RawConfigRow> = match proxy.call("ListConfigs", &()) {
            Ok(rows) => rows,
            Err(err) => {
                let err = anyhow::Error::from(err).context("D-Bus ListConfigs failed");
                if is_dbus_permission_error(&err) {
                    self.needs_elevation.store(true, Ordering::Relaxed);
                    return self.cli_fallback.list_configs();
                }
                return Err(err);
            }
        };
        Ok(rows
            .into_iter()
            .map(|(name, subvolume, _)| ConfigInfo { name, subvolume })
            .collect())
    }

    fn list_snapshots(
        &self,
        config: Option<&str>,
        include_used_space: bool,
    ) -> Result<Vec<Snapshot>> {
        if self.needs_elevation.load(Ordering::Relaxed) {
            return self.cli_fallback.list_snapshots(config, include_used_space);
        }
        let res = (|| -> Result<Vec<Snapshot>> {
            let proxy = self.proxy()?;
            let configs = self.list_configs()?;
            let subvol_by_cfg: HashMap<String, String> = configs
                .iter()
                .map(|c| (c.name.clone(), c.subvolume.clone()))
                .collect();

            match config {
                Some(cfg) => {
                    let subvol = subvol_by_cfg.get(cfg).cloned().unwrap_or_default();
                    self.list_snapshots_for_config(&proxy, cfg, &subvol, include_used_space)
                }
                None => {
                    let mut all = Vec::new();
                    for cfg in &configs {
                        let mut snaps = self.list_snapshots_for_config(
                            &proxy,
                            &cfg.name,
                            &cfg.subvolume,
                            include_used_space,
                        )?;
                        all.append(&mut snaps);
                    }
                    Ok(all)
                }
            }
        })();

        match res {
            Ok(snaps) => Ok(snaps),
            Err(err) if is_dbus_permission_error(&err) => {
                self.needs_elevation.store(true, Ordering::Relaxed);
                self.cli_fallback.list_snapshots(config, include_used_space)
            }
            Err(err) => Err(err),
        }
    }

    fn create_snapshot(&self, req: &CreateSnapshotRequest) -> Result<Option<u32>> {
        if self.needs_elevation.load(Ordering::Relaxed) {
            return self.cli_fallback.create_snapshot(req);
        }
        let desc = req.description.trim();
        if desc.is_empty() {
            return Err(anyhow!("Snapshot description must not be empty"));
        }
        let proxy = self.proxy()?;
        let cleanup = req
            .cleanup
            .as_ref()
            .map(|a| a.as_cli_str().to_string())
            .unwrap_or_default();
        let userdata: HashMap<String, String> = req
            .userdata
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        let call_res: Result<u32> = match req.snapshot_type {
            SnapshotCreateType::Single => proxy
                .call(
                    "CreateSingleSnapshot",
                    &(req.config.as_str(), desc, cleanup.as_str(), userdata),
                )
                .context("D-Bus CreateSingleSnapshot failed"),
            SnapshotCreateType::Pre => proxy
                .call(
                    "CreatePreSnapshot",
                    &(req.config.as_str(), desc, cleanup.as_str(), userdata),
                )
                .context("D-Bus CreatePreSnapshot failed"),
            SnapshotCreateType::Post { pre_number } => proxy
                .call(
                    "CreatePostSnapshot",
                    &(
                        req.config.as_str(),
                        pre_number,
                        desc,
                        cleanup.as_str(),
                        userdata,
                    ),
                )
                .context("D-Bus CreatePostSnapshot failed"),
        };

        match call_res {
            Ok(num) => Ok(Some(num)),
            Err(err) if is_dbus_permission_error(&err) => {
                self.needs_elevation.store(true, Ordering::Relaxed);
                self.cli_fallback.create_snapshot(req)
            }
            Err(err) => Err(err),
        }
    }

    fn modify_snapshot(&self, req: &ModifySnapshotRequest) -> Result<()> {
        if self.needs_elevation.load(Ordering::Relaxed) {
            return self.cli_fallback.modify_snapshot(req);
        }
        let current = self
            .list_snapshots(Some(&req.key.config), false)?
            .into_iter()
            .find(|s| s.number == req.key.number)
            .ok_or_else(|| anyhow!("Snapshot {} not found", req.key))?;

        let description = req
            .description
            .clone()
            .unwrap_or(current.description);
        let cleanup = match &req.cleanup {
            Some(Some(alg)) => alg.as_cli_str().to_string(),
            Some(None) => String::new(),
            None => current.cleanup.unwrap_or_default(),
        };
        let userdata: HashMap<String, String> = req
            .userdata
            .clone()
            .or(current.userdata)
            .unwrap_or_default()
            .into_iter()
            .collect();

        let proxy = self.proxy()?;
        let res: Result<()> = proxy
            .call(
                "SetSnapshot",
                &(
                    req.key.config.as_str(),
                    req.key.number,
                    description.as_str(),
                    cleanup.as_str(),
                    userdata,
                ),
            )
            .with_context(|| format!("D-Bus SetSnapshot({}) failed", req.key));

        match res {
            Ok(()) => Ok(()),
            Err(err) if is_dbus_permission_error(&err) => {
                self.needs_elevation.store(true, Ordering::Relaxed);
                self.cli_fallback.modify_snapshot(req)
            }
            Err(err) => Err(err),
        }
    }

    fn delete_snapshots(&self, targets: &[SnapshotKey]) -> BatchDeleteReport {
        if self.needs_elevation.load(Ordering::Relaxed) {
            return self.cli_fallback.delete_snapshots(targets);
        }
        let mut report = BatchDeleteReport::default();
        let Ok(proxy) = self.proxy() else {
            return self.cli_fallback.delete_snapshots(targets);
        };

        let mut by_config: BTreeMap<String, Vec<u32>> = BTreeMap::new();
        for target in targets {
            if target.number == 0 {
                report.failed.push((
                    target.clone(),
                    "Snapshot #0 is live system state and cannot be deleted".to_string(),
                ));
                continue;
            }
            by_config
                .entry(target.config.clone())
                .or_default()
                .push(target.number);
        }

        let mut remaining_for_cli: Vec<SnapshotKey> = Vec::new();
        for (config, nums) in by_config {
            match proxy.call::<_, _, ()>("DeleteSnapshots", &(config.as_str(), nums.as_slice())) {
                Ok(()) => {
                    for n in nums {
                        report.deleted.push(SnapshotKey::new(config.clone(), n));
                    }
                }
                Err(batch_err) => {
                    if is_dbus_permission_error(&anyhow::Error::from(batch_err)) {
                        self.needs_elevation.store(true, Ordering::Relaxed);
                        for n in nums {
                            remaining_for_cli.push(SnapshotKey::new(config.clone(), n));
                        }
                    } else {
                        // Fall back to one-by-one deletion so active/default errors are isolated per snapshot.
                        for n in nums {
                            let key = SnapshotKey::new(config.clone(), n);
                            match proxy.call::<_, _, ()>("DeleteSnapshots", &(config.as_str(), &[n][..])) {
                                Ok(()) => report.deleted.push(key),
                                Err(e) => {
                                    let err = anyhow::Error::from(e);
                                    if is_dbus_permission_error(&err) {
                                        self.needs_elevation.store(true, Ordering::Relaxed);
                                        remaining_for_cli.push(key);
                                    } else {
                                        report.failed.push((key, err.to_string()));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if !remaining_for_cli.is_empty() {
            let cli_report = self.cli_fallback.delete_snapshots(&remaining_for_cli);
            report.deleted.extend(cli_report.deleted);
            report.failed.extend(cli_report.failed);
        }
        report
    }

    fn rollback_snapshot(&self, key: &SnapshotKey) -> Result<()> {
        self.cli_fallback.rollback_snapshot(key)
    }

    fn snapshot_status(&self, config: &str, from: u32, to: u32) -> Result<Vec<FileChange>> {
        if self.needs_elevation.load(Ordering::Relaxed) {
            return self.cli_fallback.snapshot_status(config, from, to);
        }
        let res = (|| -> Result<Vec<FileChange>> {
            let proxy = self.proxy()?;
            let _: u32 = proxy
                .call("CreateComparison", &(config, from, to))
                .with_context(|| format!("D-Bus CreateComparison({config}, {from}, {to}) failed"))?;

            let files_res: Result<Vec<RawFileEntry>> = proxy
                .call("GetFiles", &(config, from, to))
                .with_context(|| format!("D-Bus GetFiles({config}, {from}, {to}) failed"));

            let _ = proxy.call::<_, _, ()>("DeleteComparison", &(config, from, to));

            let files = files_res?;
            Ok(files
                .into_iter()
                .map(|(path, mask)| FileChange {
                    path,
                    flags: decode_dbus_status_flags(mask),
                })
                .collect())
        })();

        match res {
            Ok(changes) => Ok(changes),
            Err(err) if is_dbus_permission_error(&err) => {
                self.needs_elevation.store(true, Ordering::Relaxed);
                self.cli_fallback.snapshot_status(config, from, to)
            }
            Err(err) => Err(err),
        }
    }

    fn snapshot_diff(
        &self,
        config: &str,
        from: u32,
        to: u32,
        files: &[String],
        max_bytes: usize,
    ) -> Result<DiffOutput> {
        self.cli_fallback
            .snapshot_diff(config, from, to, files, max_bytes)
    }

    fn undo_changes(
        &self,
        config: &str,
        from: u32,
        to: u32,
        files: &[String],
    ) -> Result<String> {
        self.cli_fallback.undo_changes(config, from, to, files)
    }

    fn get_config(&self, config: &str) -> Result<Vec<ConfigSetting>> {
        if self.needs_elevation.load(Ordering::Relaxed) {
            return self.cli_fallback.get_config(config);
        }
        let proxy = self.proxy()?;
        let res: Result<RawConfigDetail> = proxy
            .call("GetConfig", &(config,))
            .with_context(|| format!("D-Bus GetConfig({config}) failed"));

        let (_name, subvolume, map) = match res {
            Ok(detail) => detail,
            Err(err) if is_dbus_permission_error(&err) => {
                self.needs_elevation.store(true, Ordering::Relaxed);
                return self.cli_fallback.get_config(config);
            }
            Err(err) => return Err(err),
        };

        let mut merged: BTreeMap<String, String> = map.into_iter().collect();
        merged.entry("SUBVOLUME".to_string()).or_insert(subvolume);

        Ok(merged
            .into_iter()
            .map(|(key, value)| ConfigSetting { key, value })
            .collect())
    }

    fn set_config(&self, config: &str, settings: &[ConfigSetting]) -> Result<()> {
        if self.needs_elevation.load(Ordering::Relaxed) {
            return self.cli_fallback.set_config(config, settings);
        }
        let mut updates: HashMap<String, String> = HashMap::new();
        for setting in settings {
            let key = setting.key.trim();
            if key.is_empty() || key.contains('=') {
                return Err(anyhow!("Invalid configuration key '{}'", setting.key));
            }
            if setting.is_readonly() {
                continue;
            }
            updates.insert(key.to_string(), setting.value.clone());
        }
        if updates.is_empty() {
            return Ok(());
        }
        let proxy = self.proxy()?;
        let res: Result<()> = proxy
            .call("SetConfig", &(config, updates))
            .with_context(|| format!("D-Bus SetConfig({config}) failed"));

        match res {
            Ok(()) => Ok(()),
            Err(err) if is_dbus_permission_error(&err) => {
                self.needs_elevation.store(true, Ordering::Relaxed);
                self.cli_fallback.set_config(config, settings)
            }
            Err(err) => Err(err),
        }
    }

    fn run_cleanup(&self, config: &str, algorithm: CleanupAlgorithm) -> Result<()> {
        self.cli_fallback.run_cleanup(config, algorithm)
    }
}

pub fn is_dbus_permission_error(err: &anyhow::Error) -> bool {
    let lower = format!("{err:#}").to_lowercase();
    lower.contains("error.no_permissions")
        || lower.contains("no permissions")
        || lower.contains("accessdenied")
        || lower.contains("access denied")
        || lower.contains("not authorized")
        || lower.contains("permission denied")
}

fn decode_dbus_u64(val: &OwnedValue) -> Option<u64> {
    u64::try_from(val)
        .ok()
        .or_else(|| u32::try_from(val).ok().map(u64::from))
        .or_else(|| i64::try_from(val).ok().filter(|&v| v >= 0).map(|v| v as u64))
}

fn load_passwd_uids() -> HashMap<u32, String> {
    let mut map = HashMap::new();
    let Ok(raw) = std::fs::read_to_string("/etc/passwd") else {
        return map;
    };
    for line in raw.lines() {
        let mut parts = line.split(':');
        let Some(name) = parts.next() else {
            continue;
        };
        let _pw = parts.next();
        let Some(uid_str) = parts.next() else {
            continue;
        };
        if let Ok(uid) = uid_str.parse::<u32>() {
            map.insert(uid, name.to_string());
        }
    }
    map
}

/// Decode libsnapper `snapper::File::status_t` bitmask from `GetFiles(config, from, to)`.
///
/// Bit definitions in libsnapper (`snapper/File.h`):
/// - `0x01`: CREATED (`+`)
/// - `0x02`: DELETED (`-`)
/// - `0x04`: TYPE (`t` / replaced)
/// - `0x08`: CONTENT (`c`)
/// - `0x10`: PERMISSIONS (`p`)
/// - `0x20`: OWNER (`u`)
/// - `0x40`: GROUP (`g`)
/// - `0x80`: XATTR (`x`)
/// - `0x100`: ACL (`a`)
pub fn decode_dbus_status_flags(mask: u32) -> FileStatusFlags {
    let content = if mask & 0x01 != 0 {
        ContentChange::Created
    } else if mask & 0x02 != 0 {
        ContentChange::Deleted
    } else if mask & (0x04 | 0x08) != 0 {
        ContentChange::Modified
    } else {
        ContentChange::Unchanged
    };

    FileStatusFlags {
        content,
        permissions: mask & 0x10 != 0,
        owner: mask & 0x20 != 0,
        group: mask & 0x40 != 0,
        xattr: mask & 0x80 != 0,
        acl: mask & 0x100 != 0,
    }
}

/// Pure-Rust UTC formatting for Unix epoch seconds (Howard Hinnant's civil-from-days algorithm).
pub fn format_unix_timestamp(epoch_secs: i64) -> String {
    let days = epoch_secs.div_euclid(86_400);
    let secs_of_day = epoch_secs.rem_euclid(86_400);
    let hour = secs_of_day / 3_600;
    let minute = (secs_of_day % 3_600) / 60;
    let second = secs_of_day % 60;

    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = y + i64::from(m <= 2);

    format!("{year:04}-{m:02}-{d:02} {hour:02}:{minute:02}:{second:02} UTC")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_unix_timestamps_accurately() {
        assert_eq!(format_unix_timestamp(0), "1970-01-01 00:00:00 UTC");
        assert_eq!(
            format_unix_timestamp(1_700_000_000),
            "2023-11-14 22:13:20 UTC"
        );
        // Leap day check: 2024-02-29 12:34:56 UTC
        assert_eq!(
            format_unix_timestamp(1_709_210_096),
            "2024-02-29 12:34:56 UTC"
        );
    }

    #[test]
    fn decodes_libsnapper_status_bitmasks() {
        let created = decode_dbus_status_flags(0x01 | 0x10);
        assert_eq!(created.content, ContentChange::Created);
        assert!(created.permissions);
        assert!(!created.owner);

        let modified = decode_dbus_status_flags(0x08 | 0x20 | 0x80 | 0x100);
        assert_eq!(modified.content, ContentChange::Modified);
        assert!(modified.owner);
        assert!(modified.xattr);
        assert!(modified.acl);

        let deleted = decode_dbus_status_flags(0x02);
        assert_eq!(deleted.content, ContentChange::Deleted);

        let attrs_only = decode_dbus_status_flags(0x10 | 0x40);
        assert_eq!(attrs_only.content, ContentChange::Unchanged);
        assert!(attrs_only.permissions);
        assert!(attrs_only.group);
    }

    #[test]
    fn detects_dbus_permission_errors() {
        let err = anyhow!("D-Bus ListSnapshots(root) failed")
            .context("error.no_permissions: org.freedesktop.DBus.Error.Failed");
        assert!(is_dbus_permission_error(&err));
        assert!(!is_dbus_permission_error(&anyhow!("Connection reset")));
    }
}
