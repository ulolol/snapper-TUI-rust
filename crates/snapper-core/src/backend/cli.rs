use crate::backend::{AuthenticationRequiredError, ElevationStrategy, SnapperBackend};
use crate::model::{
    BatchDeleteReport, CleanupAlgorithm, ConfigInfo, ConfigSetting, CreateSnapshotRequest,
    DiffOutput, FileChange, ModifySnapshotRequest, Snapshot, SnapshotCreateType, SnapshotKey,
};
use crate::parser::{
    parse_config_settings_json, parse_configs_json, parse_snapshots_json, parse_status_output,
};
use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::fmt;
use std::io::{Read, Write};
use std::process::{Command, Output, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering, compiler_fence};

const COLUMNS_FULL: &str = "config,subvolume,number,type,pre-number,post-number,date,user,cleanup,description,userdata,used-space,default,active";
const COLUMNS_FAST: &str = "config,subvolume,number,type,pre-number,post-number,date,user,cleanup,description,userdata,default,active";

struct SecretPassword(Vec<u8>);

impl SecretPassword {
    fn new(password: &str) -> Self {
        Self(password.as_bytes().to_vec())
    }

    fn clone_secret(&self) -> Self {
        Self(self.0.clone())
    }

    fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Debug for SecretPassword {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretPassword(<redacted>)")
    }
}

impl Drop for SecretPassword {
    fn drop(&mut self) {
        self.0.fill(0);
        compiler_fence(Ordering::SeqCst);
    }
}

enum ExecOutcome {
    Success(DiffOutput),
    Failed(Output),
}

/// Snapper backend invoking the `snapper` CLI binary.
#[derive(Debug)]
pub struct SnapperCliBackend {
    elevation: ElevationStrategy,
    needs_elevation: AtomicBool,
    use_pkexec: AtomicBool,
    sudo_password: Mutex<Option<SecretPassword>>,
}

impl SnapperCliBackend {
    pub fn new(elevation: ElevationStrategy) -> Self {
        Self {
            elevation,
            needs_elevation: AtomicBool::new(false),
            use_pkexec: AtomicBool::new(false),
            sudo_password: Mutex::new(None),
        }
    }

    pub fn elevation(&self) -> ElevationStrategy {
        self.elevation
    }

    /// Check if the current process has effective UID 0 (root) without `unsafe` or `libc`.
    fn is_root() -> bool {
        #[cfg(target_os = "linux")]
        {
            std::fs::read_to_string("/proc/self/status")
                .ok()
                .and_then(|status| {
                    status
                        .lines()
                        .find(|line| line.starts_with("Uid:"))
                        .and_then(|line| line.split_whitespace().nth(2))
                        .map(|euid| euid == "0")
                })
                .unwrap_or(false)
        }
        #[cfg(not(target_os = "linux"))]
        {
            false
        }
    }

    pub(crate) fn is_permission_error(stderr: &str) -> bool {
        let lower = stderr.to_lowercase();
        lower.contains("no permissions")
            || lower.contains("a password is required")
            || lower.contains("permission denied")
            || lower.contains("accessdenied")
            || lower.contains("access denied")
            || lower.contains("not authorized")
            || lower.contains("no_permissions")
            || lower.contains("must be root")
            || lower.contains("operation not permitted")
    }

    fn is_sudo_auth_failure(stderr: &str) -> bool {
        let lower = stderr.to_lowercase();
        lower.contains("sorry, try again")
            || lower.contains("incorrect password attempt")
            || lower.contains("a password is required")
            || lower.contains("no password was provided")
            || lower.contains("authentication failure")
    }

    fn is_sudoers_denied(stderr: &str) -> bool {
        let lower = stderr.to_lowercase();
        lower.contains("is not in the sudoers file")
            || lower.contains("is not allowed to run sudo")
            || lower.contains("may not run sudo")
    }

    fn pkexec_failure_reason(output: &Output) -> Option<&'static str> {
        let stderr = String::from_utf8_lossy(&output.stderr).to_lowercase();
        if stderr.contains("no authentication agent")
            || stderr.contains("request dismissed")
            || matches!(output.status.code(), Some(127))
        {
            return Some(
                "No graphical Polkit authentication agent is running in this desktop session. Enter your sudo password instead.",
            );
        }
        if matches!(output.status.code(), Some(126)) || stderr.contains("dismissed") {
            return Some("Polkit authentication was cancelled.");
        }
        None
    }

    fn map_command_failure(output: &Output, via_pkexec: bool) -> anyhow::Error {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if via_pkexec && let Some(reason) = Self::pkexec_failure_reason(output) {
            return anyhow::anyhow!("{reason}");
        }
        if stderr.contains("a password is required") {
            return anyhow::anyhow!(
                "Root privileges required (run with sudo or configure NOPASSWD)"
            );
        }
        if stderr.is_empty() {
            anyhow::anyhow!("Snapper failed with exit code {:?}", output.status.code())
        } else {
            anyhow::anyhow!("Snapper failed: {}", stderr)
        }
    }

    fn exec_command_capture(
        program: &str,
        prog_args: &[String],
        stdin_secret: Option<&[u8]>,
        max_bytes: Option<usize>,
    ) -> Result<ExecOutcome> {
        let mut cmd = Command::new(program);
        cmd.args(prog_args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if stdin_secret.is_some() {
            cmd.stdin(Stdio::piped());
        } else {
            cmd.stdin(Stdio::null());
        }

        let mut child = cmd
            .spawn()
            .with_context(|| format!("Failed to execute {} command", program))?;

        if let Some(secret) = stdin_secret
            && let Some(mut stdin) = child.stdin.take()
        {
            let mut payload = Vec::with_capacity(secret.len() + 1);
            payload.extend_from_slice(secret);
            payload.push(b'\n');
            let _ = stdin.write_all(&payload);
            let _ = stdin.flush();
            payload.fill(0);
            compiler_fence(Ordering::SeqCst);
            drop(stdin);
        }

        let stderr_pipe = child.stderr.take();
        let stderr_handle = std::thread::spawn(move || -> Vec<u8> {
            let mut err_buf = Vec::new();
            if let Some(mut err) = stderr_pipe {
                let _ = err.read_to_end(&mut err_buf);
            }
            err_buf
        });

        let mut buf = Vec::new();
        if let Some(stdout) = child.stdout.take() {
            if let Some(limit) = max_bytes {
                let cap = u64::try_from(limit).unwrap_or(u64::MAX).saturating_add(1);
                stdout
                    .take(cap)
                    .read_to_end(&mut buf)
                    .context("Failed reading snapper stdout")?;
            } else {
                let mut reader = stdout;
                reader
                    .read_to_end(&mut buf)
                    .context("Failed reading snapper stdout")?;
            }
        }

        if let Some(limit) = max_bytes
            && buf.len() > limit
        {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stderr_handle.join();
            let total_bytes = buf.len();
            let patch = truncate_patch_at_line_boundary(&buf, limit);
            return Ok(ExecOutcome::Success(DiffOutput {
                patch,
                truncated: true,
                total_bytes,
            }));
        }

        let status = child.wait().context("Failed waiting for snapper process")?;
        let stderr_buf = stderr_handle.join().unwrap_or_default();
        if !status.success() {
            return Ok(ExecOutcome::Failed(Output {
                status,
                stdout: buf,
                stderr: stderr_buf,
            }));
        }

        let total_bytes = buf.len();
        let patch = String::from_utf8_lossy(&buf).into_owned();
        Ok(ExecOutcome::Success(DiffOutput {
            patch,
            truncated: false,
            total_bytes,
        }))
    }

    fn cached_password(&self) -> Option<SecretPassword> {
        self.sudo_password
            .lock()
            .ok()
            .and_then(|guard| guard.as_ref().map(SecretPassword::clone_secret))
    }

    fn clear_cached_password(&self) {
        if let Ok(mut guard) = self.sudo_password.lock() {
            *guard = None;
        }
    }

    fn pkexec_snapper_args(args: &[String]) -> Vec<String> {
        let mut pkexec_args = Vec::with_capacity(args.len() + 2);
        pkexec_args.push("--disable-internal-agent".to_string());
        pkexec_args.push("snapper".to_string());
        pkexec_args.extend(args.iter().cloned());
        pkexec_args
    }

    fn sudo_password_snapper_args(args: &[String]) -> Vec<String> {
        let mut sudo_args = Vec::with_capacity(args.len() + 4);
        sudo_args.push("-S".to_string());
        sudo_args.push("-k".to_string());
        sudo_args.push("-p".to_string());
        sudo_args.push(String::new());
        sudo_args.push("snapper".to_string());
        sudo_args.extend(args.iter().cloned());
        sudo_args
    }

    fn run_sudo_with_password_impl(
        &self,
        args: &[String],
        password: &[u8],
        max_bytes: Option<usize>,
    ) -> Result<DiffOutput> {
        let sudo_args = Self::sudo_password_snapper_args(args);
        match Self::exec_command_capture("sudo", &sudo_args, Some(password), max_bytes)? {
            ExecOutcome::Success(out) => Ok(out),
            ExecOutcome::Failed(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                if Self::is_sudoers_denied(&stderr) {
                    self.clear_cached_password();
                    return Err(AuthenticationRequiredError::new(
                        "Current user is not permitted to run sudo snapper (try Polkit or configure sudoers).",
                    )
                    .into());
                }
                if Self::is_sudo_auth_failure(&stderr) || Self::is_permission_error(&stderr) {
                    self.clear_cached_password();
                    return Err(AuthenticationRequiredError::new(
                        "Sudo password was rejected or expired; please re-authenticate.",
                    )
                    .into());
                }
                Err(Self::map_command_failure(&output, false))
            }
        }
    }

    fn run_pkexec_impl(&self, args: &[String], max_bytes: Option<usize>) -> Result<DiffOutput> {
        let pkexec_args = Self::pkexec_snapper_args(args);
        match Self::exec_command_capture("pkexec", &pkexec_args, None, max_bytes)? {
            ExecOutcome::Success(out) => Ok(out),
            ExecOutcome::Failed(output) => {
                if let Some(reason) = Self::pkexec_failure_reason(&output) {
                    self.use_pkexec.store(false, Ordering::Relaxed);
                    return Err(AuthenticationRequiredError::new(reason).into());
                }
                Err(Self::map_command_failure(&output, true))
            }
        }
    }

    fn run_pkexec_fallback_impl(
        &self,
        args: &[String],
        max_bytes: Option<usize>,
        try_direct_first: bool,
    ) -> Result<DiffOutput> {
        if Self::is_root() {
            return match Self::exec_command_capture("snapper", args, None, max_bytes)? {
                ExecOutcome::Success(out) => Ok(out),
                ExecOutcome::Failed(output) => Err(Self::map_command_failure(&output, false)),
            };
        }

        if let Some(pw) = self.cached_password() {
            return self.run_sudo_with_password_impl(args, pw.as_bytes(), max_bytes);
        }

        if self.use_pkexec.load(Ordering::Relaxed) {
            return self.run_pkexec_impl(args, max_bytes);
        }

        if !self.needs_elevation.load(Ordering::Relaxed) || try_direct_first {
            match Self::exec_command_capture("snapper", args, None, max_bytes)? {
                ExecOutcome::Success(out) => return Ok(out),
                ExecOutcome::Failed(output) => {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    if !Self::is_permission_error(&stderr) {
                        return Err(Self::map_command_failure(&output, false));
                    }
                    self.needs_elevation.store(true, Ordering::Relaxed);
                }
            }
        }

        let mut sudo_n_args = Vec::with_capacity(args.len() + 2);
        sudo_n_args.push("-n".to_string());
        sudo_n_args.push("snapper".to_string());
        sudo_n_args.extend(args.iter().cloned());
        if let ExecOutcome::Success(out) =
            Self::exec_command_capture("sudo", &sudo_n_args, None, max_bytes)?
        {
            return Ok(out);
        }

        Err(AuthenticationRequiredError::new(
            "Administrator privileges are required to access Snapper snapshots.",
        )
        .into())
    }

    fn run_snapper(&self, args: &[String], try_direct_first: bool) -> Result<String> {
        self.run_snapper_internal(args, None, try_direct_first)
            .map(|d| d.patch)
    }

    fn run_snapper_bounded(
        &self,
        args: &[String],
        max_bytes: usize,
        try_direct_first: bool,
    ) -> Result<DiffOutput> {
        self.run_snapper_internal(args, Some(max_bytes), try_direct_first)
    }

    fn run_snapper_internal(
        &self,
        args: &[String],
        max_bytes: Option<usize>,
        try_direct_first: bool,
    ) -> Result<DiffOutput> {
        let is_root = Self::is_root();
        match self.elevation {
            ElevationStrategy::Direct => {
                match Self::exec_command_capture("snapper", args, None, max_bytes)? {
                    ExecOutcome::Success(out) => Ok(out),
                    ExecOutcome::Failed(output) => Err(Self::map_command_failure(&output, false)),
                }
            }
            ElevationStrategy::SudoNonInteractive => {
                if is_root {
                    return match Self::exec_command_capture("snapper", args, None, max_bytes)? {
                        ExecOutcome::Success(out) => Ok(out),
                        ExecOutcome::Failed(output) => {
                            Err(Self::map_command_failure(&output, false))
                        }
                    };
                }

                if try_direct_first {
                    match Self::exec_command_capture("snapper", args, None, max_bytes)? {
                        ExecOutcome::Success(out) => return Ok(out),
                        ExecOutcome::Failed(output) => {
                            let stderr = String::from_utf8_lossy(&output.stderr);
                            if !Self::is_permission_error(&stderr) {
                                return Err(Self::map_command_failure(&output, false));
                            }
                        }
                    }
                }

                let mut sudo_args = Vec::with_capacity(args.len() + 2);
                sudo_args.push("-n".to_string());
                sudo_args.push("snapper".to_string());
                sudo_args.extend(args.iter().cloned());
                match Self::exec_command_capture("sudo", &sudo_args, None, max_bytes)? {
                    ExecOutcome::Success(out) => Ok(out),
                    ExecOutcome::Failed(output) => Err(Self::map_command_failure(&output, false)),
                }
            }
            ElevationStrategy::PkexecFallback => {
                self.run_pkexec_fallback_impl(args, max_bytes, try_direct_first)
            }
        }
    }

    fn build_list_args(config: Option<&str>, include_used_space: bool) -> Vec<String> {
        let mut args = vec!["--jsonout".to_string()];
        if let Some(cfg) = config {
            Self::push_config_arg(&mut args, cfg);
        }
        args.push("list".to_string());
        if config.is_none() {
            args.push("-a".to_string());
        }
        if !include_used_space {
            args.push("--disable-used-space".to_string());
        }
        args.push("--columns".to_string());
        args.push(if include_used_space {
            COLUMNS_FULL.to_string()
        } else {
            COLUMNS_FAST.to_string()
        });
        args
    }

    fn format_userdata(map: &BTreeMap<String, String>) -> String {
        map.iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join(",")
    }

    fn push_config_arg(args: &mut Vec<String>, config: &str) {
        if !config.trim().is_empty() {
            args.push("-c".to_string());
            args.push(config.to_string());
        }
    }
}

/// Truncate raw diff bytes cleanly on a line boundary (or UTF-8 boundary if a line exceeds `max_bytes`).
pub(crate) fn truncate_patch_at_line_boundary(buf: &[u8], max_bytes: usize) -> String {
    let limit = buf.len().min(max_bytes);
    let slice = &buf[..limit];
    let cut = slice
        .iter()
        .rposition(|&b| b == b'\n')
        .map(|pos| pos + 1)
        .filter(|&pos| pos > 0)
        .unwrap_or(limit);
    let lossy = String::from_utf8_lossy(&buf[..cut]);
    // Trim trailing U+FFFD if a non-newline cut bisected a multi-byte UTF-8 sequence.
    lossy.trim_end_matches('\u{FFFD}').to_string()
}

impl SnapperBackend for SnapperCliBackend {
    fn backend_label(&self) -> String {
        match self.elevation {
            ElevationStrategy::Direct => "CLI (Direct)".to_string(),
            ElevationStrategy::SudoNonInteractive => "CLI (sudo -n)".to_string(),
            ElevationStrategy::PkexecFallback => {
                if Self::is_root() {
                    "CLI (Root)".to_string()
                } else if self.cached_password().is_some() {
                    "CLI (sudo session)".to_string()
                } else if self.use_pkexec.load(Ordering::Relaxed) {
                    "CLI (Polkit)".to_string()
                } else if self.needs_elevation.load(Ordering::Relaxed) {
                    "CLI (Auth Required)".to_string()
                } else {
                    "CLI (Direct/Auth)".to_string()
                }
            }
        }
    }

    fn prefers_single_phase_list(&self) -> bool {
        self.elevation == ElevationStrategy::PkexecFallback
            && !Self::is_root()
            && self.use_pkexec.load(Ordering::Relaxed)
    }

    fn supports_password_auth(&self) -> bool {
        self.elevation == ElevationStrategy::PkexecFallback && !Self::is_root()
    }

    fn is_authenticated(&self) -> bool {
        Self::is_root()
            || self.cached_password().is_some()
            || self.use_pkexec.load(Ordering::Relaxed)
    }

    fn authenticate_with_password(&self, password: &str) -> Result<()> {
        if password.is_empty() {
            anyhow::bail!("Password cannot be empty.");
        }
        let check_args = Self::sudo_password_snapper_args(&["--version".to_string()]);
        match Self::exec_command_capture("sudo", &check_args, Some(password.as_bytes()), None)? {
            ExecOutcome::Success(_) => {
                if let Ok(mut guard) = self.sudo_password.lock() {
                    *guard = Some(SecretPassword::new(password));
                }
                self.use_pkexec.store(false, Ordering::Relaxed);
                self.needs_elevation.store(true, Ordering::Relaxed);
                Ok(())
            }
            ExecOutcome::Failed(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                if Self::is_sudoers_denied(&stderr) {
                    anyhow::bail!(
                        "User is not allowed to run sudo for snapper (try Polkit or configure sudoers)."
                    );
                }
                if Self::is_sudo_auth_failure(&stderr) {
                    anyhow::bail!("Incorrect password. Please try again.");
                }
                Err(Self::map_command_failure(&output, false))
            }
        }
    }

    fn enable_pkexec_mode(&self) -> Result<()> {
        let check_args = Self::pkexec_snapper_args(&["--version".to_string()]);
        match Self::exec_command_capture("pkexec", &check_args, None, None)? {
            ExecOutcome::Success(_) => {
                self.clear_cached_password();
                self.use_pkexec.store(true, Ordering::Relaxed);
                self.needs_elevation.store(true, Ordering::Relaxed);
                Ok(())
            }
            ExecOutcome::Failed(output) => {
                if let Some(reason) = Self::pkexec_failure_reason(&output) {
                    anyhow::bail!("{reason}");
                }
                Err(Self::map_command_failure(&output, true))
            }
        }
    }

    fn clear_authentication(&self) {
        self.clear_cached_password();
        self.use_pkexec.store(false, Ordering::Relaxed);
    }

    fn list_configs(&self) -> Result<Vec<ConfigInfo>> {
        let args = vec!["--jsonout".to_string(), "list-configs".to_string()];
        let out = self.run_snapper(&args, true)?;
        parse_configs_json(&out)
    }

    fn list_snapshots(
        &self,
        config: Option<&str>,
        include_used_space: bool,
    ) -> Result<Vec<Snapshot>> {
        let effective_used_space = include_used_space || self.prefers_single_phase_list();
        let args = Self::build_list_args(config, effective_used_space);
        let out = self.run_snapper(&args, true)?;
        parse_snapshots_json(&out)
    }

    fn create_snapshot(&self, req: &CreateSnapshotRequest) -> Result<Option<u32>> {
        let mut args = Vec::new();
        Self::push_config_arg(&mut args, &req.config);
        args.push("create".to_string());
        args.push("--print-number".to_string());
        args.push("--type".to_string());
        args.push(req.snapshot_type.as_cli_str().to_string());
        if let SnapshotCreateType::Post { pre_number } = req.snapshot_type {
            args.push("--pre-number".to_string());
            args.push(pre_number.to_string());
        }
        if let Some(alg) = &req.cleanup {
            args.push("--cleanup-algorithm".to_string());
            args.push(alg.as_cli_str().to_string());
        }
        if !req.userdata.is_empty() {
            args.push("--userdata".to_string());
            args.push(Self::format_userdata(&req.userdata));
        }
        args.push("--description".to_string());
        args.push(req.description.clone());

        let out = self
            .run_snapper(&args, false)
            .context("Failed to create snapshot")?;
        Ok(out.trim().parse::<u32>().ok())
    }

    fn modify_snapshot(&self, req: &ModifySnapshotRequest) -> Result<()> {
        let mut args = Vec::new();
        Self::push_config_arg(&mut args, &req.key.config);
        args.push("modify".to_string());

        if let Some(desc) = &req.description {
            args.push("--description".to_string());
            args.push(desc.clone());
        }
        if let Some(cleanup_opt) = &req.cleanup {
            args.push("--cleanup-algorithm".to_string());
            match cleanup_opt {
                Some(alg) => args.push(alg.as_cli_str().to_string()),
                None => args.push(String::new()),
            }
        }
        if let Some(ud) = &req.userdata {
            args.push("--userdata".to_string());
            args.push(Self::format_userdata(ud));
        }
        args.push(req.key.number.to_string());

        self.run_snapper(&args, false)
            .with_context(|| format!("Failed to modify snapshot {}", req.key))?;
        Ok(())
    }

    fn delete_snapshots(&self, targets: &[SnapshotKey]) -> BatchDeleteReport {
        let mut report = BatchDeleteReport::default();
        for key in targets {
            if key.number == 0 {
                report.failed.push((
                    key.clone(),
                    "Snapshot #0 (current system) cannot be deleted".to_string(),
                ));
                continue;
            }
            let mut args = Vec::new();
            Self::push_config_arg(&mut args, &key.config);
            args.push("delete".to_string());
            args.push(key.number.to_string());
            match self.run_snapper(&args, false) {
                Ok(_) => report.deleted.push(key.clone()),
                Err(err) => report.failed.push((key.clone(), err.to_string())),
            }
        }
        report
    }

    fn rollback_snapshot(&self, key: &SnapshotKey) -> Result<()> {
        let mut args = Vec::new();
        Self::push_config_arg(&mut args, &key.config);
        args.push("rollback".to_string());
        args.push(key.number.to_string());
        self.run_snapper(&args, false)
            .with_context(|| format!("Failed to rollback to snapshot {}", key))?;
        Ok(())
    }

    fn snapshot_status(&self, config: &str, from: u32, to: u32) -> Result<Vec<FileChange>> {
        let mut args = Vec::new();
        Self::push_config_arg(&mut args, config);
        args.push("status".to_string());
        args.push(format!("{}..{}", from, to));
        let out = self.run_snapper(&args, true)?;
        Ok(parse_status_output(&out))
    }

    fn snapshot_diff(
        &self,
        config: &str,
        from: u32,
        to: u32,
        files: &[String],
        max_bytes: usize,
    ) -> Result<DiffOutput> {
        let mut args = Vec::new();
        Self::push_config_arg(&mut args, config);
        args.push("diff".to_string());
        args.push(format!("{}..{}", from, to));
        if !files.is_empty() {
            args.push("--".to_string());
            args.extend(files.iter().cloned());
        }
        self.run_snapper_bounded(&args, max_bytes, true)
    }

    fn undo_changes(
        &self,
        config: &str,
        from: u32,
        to: u32,
        files: &[String],
    ) -> Result<String> {
        let mut args = Vec::new();
        Self::push_config_arg(&mut args, config);
        args.push("undochange".to_string());
        args.push(format!("{}..{}", from, to));
        if !files.is_empty() {
            args.push("--".to_string());
            args.extend(files.iter().cloned());
        }
        self.run_snapper(&args, false)
            .context("Failed to restore files with snapper undochange")
    }

    fn get_config(&self, config: &str) -> Result<Vec<ConfigSetting>> {
        let mut args = vec!["--jsonout".to_string()];
        Self::push_config_arg(&mut args, config);
        args.push("get-config".to_string());
        let out = self.run_snapper(&args, true)?;
        parse_config_settings_json(&out)
    }

    fn set_config(&self, config: &str, settings: &[ConfigSetting]) -> Result<()> {
        if settings.is_empty() {
            return Ok(());
        }
        let mut args = Vec::new();
        Self::push_config_arg(&mut args, config);
        args.push("set-config".to_string());
        let mut any_editable = false;
        for s in settings {
            if s.is_readonly() {
                continue;
            }
            if s.key.is_empty() || s.key.contains('=') || s.key.chars().any(char::is_whitespace) {
                anyhow::bail!("Invalid Snapper config key: {:?}", s.key);
            }
            args.push(format!("{}={}", s.key, s.value));
            any_editable = true;
        }
        if !any_editable {
            return Ok(());
        }
        self.run_snapper(&args, false)
            .with_context(|| format!("Failed to update config '{}'", config))?;
        Ok(())
    }

    fn run_cleanup(&self, config: &str, algorithm: CleanupAlgorithm) -> Result<()> {
        let mut args = Vec::new();
        Self::push_config_arg(&mut args, config);
        args.push("cleanup".to_string());
        args.push(algorithm.as_cli_str().to_string());
        self.run_snapper(&args, false)
            .with_context(|| format!("Failed to run '{}' cleanup on '{}'", algorithm, config))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_error_detection() {
        assert!(SnapperCliBackend::is_permission_error("No permissions."));
        assert!(SnapperCliBackend::is_permission_error(
            "org.freedesktop.DBus.Error.AccessDenied"
        ));
        assert!(SnapperCliBackend::is_permission_error(
            "sudo: a password is required"
        ));
        assert!(!SnapperCliBackend::is_permission_error(
            "Config 'foo' not found"
        ));
    }

    #[test]
    fn test_list_args_include_all_configs_and_columns() {
        let fast = SnapperCliBackend::build_list_args(None, false);
        assert!(fast.contains(&"-a".to_string()));
        assert!(fast.contains(&"--disable-used-space".to_string()));
        assert!(!fast.last().unwrap().contains("used-space"));

        let single = SnapperCliBackend::build_list_args(Some("home"), true);
        assert!(!single.contains(&"-a".to_string()));
        assert!(!single.contains(&"--disable-used-space".to_string()));
        assert!(single.last().unwrap().contains("used-space"));
    }

    #[test]
    fn test_truncate_patch_at_line_boundary() {
        let input = b"--- a/file\n+++ b/file\n@@ -1 +1 @@\n-old\n+new line here\n";
        let cut = truncate_patch_at_line_boundary(input, 35);
        assert_eq!(cut, "--- a/file\n+++ b/file\n@@ -1 +1 @@\n");

        // Multi-byte UTF-8 without newline fallback
        let utf8 = "abcdef🚀".as_bytes();
        let cut_utf8 = truncate_patch_at_line_boundary(utf8, 8);
        assert_eq!(cut_utf8, "abcdef");
    }

    #[test]
    fn test_secret_password_redacted_and_auth_error_detection() {
        let secret = SecretPassword::new("super-secret");
        assert_eq!(format!("{secret:?}"), "SecretPassword(<redacted>)");
        assert_eq!(secret.as_bytes(), b"super-secret");

        let direct: anyhow::Error = AuthenticationRequiredError::new("auth needed").into();
        assert!(crate::backend::is_authentication_required(&direct));
        let wrapped = direct.context("Wrapper context");
        assert!(crate::backend::is_authentication_required(&wrapped));

        assert!(SnapperCliBackend::is_sudo_auth_failure(
            "Sorry, try again.\nsudo: 1 incorrect password attempt"
        ));
        assert!(SnapperCliBackend::is_sudo_auth_failure(
            "sudo: a password is required"
        ));
        assert!(!SnapperCliBackend::is_sudo_auth_failure(
            "Config 'root' not found"
        ));
    }
}
