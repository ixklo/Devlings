//! In-app updates (M1). Checks the signed `latest.json` on GitHub Releases with
//! tauri-plugin-updater, downloads an update in the background, and installs it
//! when the user clicks Restart. Everything runs in Rust; the webview has no
//! updater permissions and only sees `Snapshot.update`.

use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex, MutexGuard,
};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Manager, Url};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::state::{self, now_ms, AppState, Snapshot};

/// Points the updater at another `latest.json`, e.g. a pre-release's asset, since
/// `/releases/latest` ignores pre-releases. Signatures are still checked.
pub const ENDPOINT_ENV: &str = "PERCH_UPDATE_ENDPOINT";
/// Check this often while auto-update is on (after the launch check).
pub const CHECK_EVERY_MS: i64 = 24 * 60 * 60 * 1000;
/// After a failed check or download, retry this soon, doubling per failure up to `CHECK_EVERY_MS`.
pub const RETRY_FIRST_MS: i64 = 60 * 60 * 1000;
const LAUNCH_DELAY: Duration = Duration::from_secs(30);
const POLL_EVERY: Duration = Duration::from_secs(10 * 60);
const CHECK_TIMEOUT: Duration = Duration::from_secs(30);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(15 * 60);
const NOTES_MAX_CHARS: usize = 2000;
const BUNDLE_PREFIX: &str = "perch-";
const BUNDLE_SUFFIX: &str = ".update";

pub const ASK_RUNNING: &str = "Finish or stop the running ask first.";
pub const NOT_READY: &str = "No update is ready to install yet.";
pub const RESTARTING: &str = "Perch is restarting to install an update.";
pub const CHECK_FAILED: &str = "Couldn't check for updates.";
pub const DOWNLOAD_FAILED: &str = "Couldn't download the update.";
pub const BAD_SIGNATURE: &str = "The update's signature didn't match, so Perch won't install it.";
pub const DEV_BUILD: &str = "Updates are off in development builds.";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum UpdateState {
    #[default]
    Idle,
    Checking,
    Available,
    Downloading,
    Ready,
    Error,
    Disabled,
}

/// `Snapshot.update`. Optional fields are left out of the JSON when unset.
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    pub state: UpdateState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    /// Whole percent, 0–100; absent when the server sends no length.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Something that happened to the update flow.
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    CheckStarted,
    UpToDate,
    Found { version: String, notes: Option<String> },
    Progress { received: u64, total: Option<u64> },
    Downloaded { version: String, notes: Option<String> },
    CheckFailed { manual: bool },
    DownloadFailed { manual: bool, bad_signature: bool },
}

impl UpdateStatus {
    fn with_state(state: UpdateState) -> Self {
        Self { state, ..Self::default() }
    }

    fn disabled(reason: &str) -> Self {
        Self { error: Some(reason.to_string()), ..Self::with_state(UpdateState::Disabled) }
    }

    fn failed(message: &str) -> Self {
        Self { error: Some(message.to_string()), ..Self::with_state(UpdateState::Error) }
    }

    pub fn apply(&self, step: Step) -> UpdateStatus {
        use UpdateState::*;
        match (self.state, step) {
            (Disabled, _) => self.clone(),
            // A ready update stays offered while later checks run; only a newer, fully
            // downloaded and verified update replaces it. Failures meanwhile are logged only.
            (Ready, Step::Downloaded { version, notes }) => Self { version: Some(version), notes, ..Self::with_state(Ready) },
            (Ready, _) => self.clone(),
            (Checking | Available | Downloading, Step::CheckStarted) => self.clone(),
            (_, Step::CheckStarted) => Self::with_state(Checking),
            (_, Step::UpToDate) => Self::with_state(Idle),
            (_, Step::Found { version, notes }) => Self { version: Some(version), notes, ..Self::with_state(Available) },
            (_, Step::Progress { received, total }) => Self {
                state: Downloading,
                progress: percent(received, total),
                error: None,
                ..self.clone()
            },
            (_, Step::Downloaded { version, notes }) => Self { version: Some(version), notes, ..Self::with_state(Ready) },
            (_, Step::DownloadFailed { bad_signature: true, .. }) => Self::failed(BAD_SIGNATURE),
            (_, Step::CheckFailed { manual: true }) => Self::failed(CHECK_FAILED),
            (_, Step::DownloadFailed { manual: true, .. }) => Self::failed(DOWNLOAD_FAILED),
            (_, Step::CheckFailed { manual: false } | Step::DownloadFailed { manual: false, .. }) => Self::with_state(Idle),
        }
    }
}

pub fn percent(received: u64, total: Option<u64>) -> Option<u8> {
    match total {
        Some(total) if total > 0 => Some((u128::from(received) * 100 / u128::from(total)).min(100) as u8),
        _ => None,
    }
}

/// Consecutive failed checks or downloads since the last success (in memory; every launch checks anyway).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Backoff {
    pub failures: u32,
    pub last_failure: Option<i64>,
}

impl Backoff {
    pub fn failed(self, now: i64) -> Self {
        Self { failures: self.failures.saturating_add(1), last_failure: Some(now) }
    }

    /// 1 h after the first failure, then 2 h, 4 h, … up to a day.
    pub fn retry_delay_ms(&self) -> i64 {
        let doublings = self.failures.saturating_sub(1).min(16);
        RETRY_FIRST_MS.saturating_mul(1 << doublings).min(CHECK_EVERY_MS)
    }
}

/// Whether the scheduler should check now. `first` is the check ~30 s after launch;
/// `last_success` is `lastUpdateCheck`, stamped only when a check succeeds.
pub fn check_due(auto: bool, first: bool, last_success: Option<i64>, backoff: Backoff, now: i64) -> bool {
    if !auto {
        return false;
    }
    if first {
        return true;
    }
    // A time "in the future" means the clock moved back; check rather than wait it out.
    let elapsed = |at: i64, wait: i64| now < at || now - at >= wait;
    match (backoff.failures, backoff.last_failure) {
        (1.., Some(at)) => elapsed(at, backoff.retry_delay_ms()),
        _ => last_success.is_none_or(|at| elapsed(at, CHECK_EVERY_MS)),
    }
}

/// True when `found` is a newer version than `ready` (semver). Unparseable versions never replace anything.
pub fn is_newer(found: &str, ready: &str) -> bool {
    match (semver::Version::parse(found), semver::Version::parse(ready)) {
        (Ok(f), Ok(r)) => f > r,
        _ => false,
    }
}

/// The endpoints to use instead of tauri.conf.json's, from `PERCH_UPDATE_ENDPOINT`.
pub fn endpoint_override(var: Option<&str>) -> Result<Option<Vec<Url>>, String> {
    match var.map(str::trim).filter(|v| !v.is_empty()) {
        None => Ok(None),
        Some(v) => Url::parse(v).map(|url| Some(vec![url])).map_err(|e| format!("{ENDPOINT_ENV} isn't a valid URL ({e})")),
    }
}

/// Development builds only check when pointed at a test endpoint, so `tauri dev` never installs over a real copy.
pub fn available(debug_build: bool, has_override: bool) -> bool {
    !debug_build || has_override
}

/// Why `install_update` must refuse right now, if it must.
pub fn install_refusal(status: &UpdateStatus, ask_running: bool) -> Option<&'static str> {
    if status.state != UpdateState::Ready {
        Some(NOT_READY)
    } else if ask_running {
        Some(ASK_RUNNING)
    } else {
        None
    }
}

/// Writes a verified update bundle to `dir` as `perch-<version>.update`.
pub fn save_bundle(dir: &Path, version: &str, bytes: &[u8]) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let safe: String =
        version.chars().map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '-') { c } else { '_' }).collect();
    let path = dir.join(format!("{BUNDLE_PREFIX}{safe}{BUNDLE_SUFFIX}"));
    std::fs::write(&path, bytes)?;
    Ok(path)
}

/// Deletes Perch's update bundles in `dir`, except `keep`. Other files are left alone.
pub fn remove_bundles(dir: &Path, keep: Option<&Path>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for path in entries.flatten().map(|e| e.path()) {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        let ours = name.starts_with(BUNDLE_PREFIX) && name.ends_with(BUNDLE_SUFFIX);
        if ours && Some(path.as_path()) != keep {
            if let Err(e) = std::fs::remove_file(&path) {
                log::warn!("update: couldn't remove {}: {e}", path.display());
            }
        }
    }
}

// ---- Runtime ----

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn endpoint_var() -> Option<String> {
    std::env::var(ENDPOINT_ENV).ok()
}

/// A downloaded, signature-verified update waiting on disk for a restart.
struct ReadyUpdate {
    update: Update,
    path: PathBuf,
}

/// Managed state: the status the UI sees, the ready update, and the scheduler's bookkeeping.
pub struct Updates {
    status: Mutex<UpdateStatus>,
    ready: Mutex<Option<ReadyUpdate>>,
    backoff: Mutex<Backoff>,
    /// A check or its download is running; only one at a time.
    working: AtomicBool,
    /// Set once Restart was clicked; `ask` refuses from then on.
    installing: AtomicBool,
}

impl Default for Updates {
    fn default() -> Self {
        let has_override = matches!(endpoint_override(endpoint_var().as_deref()), Ok(Some(_)));
        let status = if available(cfg!(debug_assertions), has_override) {
            UpdateStatus::default()
        } else {
            UpdateStatus::disabled(DEV_BUILD)
        };
        Self {
            status: Mutex::new(status),
            ready: Mutex::new(None),
            backoff: Mutex::new(Backoff::default()),
            working: AtomicBool::new(false),
            installing: AtomicBool::new(false),
        }
    }
}

/// Clears `Updates::working` when the check (or the download it started) ends, however it ends.
struct Working(AppHandle);

impl Drop for Working {
    fn drop(&mut self) {
        self.0.state::<Updates>().working.store(false, Ordering::SeqCst);
    }
}

pub fn status(app: &AppHandle) -> UpdateStatus {
    lock(&app.state::<Updates>().status).clone()
}

fn bundles_dir(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_cache_dir().ok().map(|d| d.join("updates"))
}

/// Applies a step and publishes a snapshot if anything the UI shows changed.
fn advance(app: &AppHandle, step: Step) -> UpdateStatus {
    let (next, changed) = {
        let updates = app.state::<Updates>();
        let mut status = lock(&updates.status);
        let next = status.apply(step);
        let changed = next != *status;
        *status = next.clone();
        (next, changed)
    };
    if changed {
        state::emit_snapshot(app);
    }
    next
}

/// A check found an update or confirmed there's none: stamp `lastUpdateCheck` and reset the backoff.
fn record_success(app: &AppHandle) {
    *lock(&app.state::<Updates>().backoff) = Backoff::default();
    let s = app.state::<AppState>();
    lock(&s.config).last_update_check = Some(now_ms());
    s.save_config();
}

fn record_failure(app: &AppHandle) {
    let updates = app.state::<Updates>();
    let mut backoff = lock(&updates.backoff);
    *backoff = backoff.failed(now_ms());
    log::info!("update: next attempt in about {} min", backoff.retry_delay_ms() / 60_000);
}

/// Clears bundles left by earlier runs, then checks ~30 s after launch and again when due
/// (a day after the last success, sooner after a failure) while auto-update is on.
pub fn start(app: AppHandle) {
    if let Some(dir) = bundles_dir(&app) {
        remove_bundles(&dir, None);
    }
    std::thread::spawn(move || {
        std::thread::sleep(LAUNCH_DELAY);
        let mut first = true;
        loop {
            let (auto, last) = {
                let s = app.state::<AppState>();
                let config = lock(&s.config);
                (config.auto_update, config.last_update_check)
            };
            let backoff = *lock(&app.state::<Updates>().backoff);
            if check_due(auto, first, last, backoff, now_ms()) {
                tauri::async_runtime::block_on(check(&app, false));
            }
            first = false;
            std::thread::sleep(POLL_EVERY);
        }
    });
}

/// Looks for an update and, when there is one (newer than any ready one), downloads it in the
/// background. Returns the status once the check itself is done. Background failures stay quiet.
pub async fn check(app: &AppHandle, manual: bool) -> UpdateStatus {
    let updates = app.state::<Updates>();
    if status(app).state == UpdateState::Disabled || updates.working.swap(true, Ordering::SeqCst) {
        return status(app);
    }
    let working = Working(app.clone());
    advance(app, Step::CheckStarted);
    let ready_version = lock(&updates.ready).as_ref().map(|r| r.update.version.clone());
    match find(app).await {
        Ok(Some(update)) if ready_version.as_deref().is_none_or(|r| is_newer(&update.version, r)) => {
            log::info!("update check: Perch {} is available", update.version);
            record_success(app);
            let notes = update.body.as_deref().map(|n| n.chars().take(NOTES_MAX_CHARS).collect());
            let found = advance(app, Step::Found { version: update.version.clone(), notes });
            let handle = app.clone();
            tauri::async_runtime::spawn(async move {
                download(&handle, update, manual).await;
                drop(working);
            });
            found
        }
        Ok(_) => {
            log::info!("update check: nothing newer than {}", ready_version.as_deref().unwrap_or("this version"));
            record_success(app);
            advance(app, Step::UpToDate)
        }
        Err(e) => {
            // Offline, a 404 (e.g. a latest release without latest.json) or a bad response.
            log::warn!("update check failed: {e}");
            record_failure(app);
            advance(app, Step::CheckFailed { manual })
        }
    }
}

async fn find(app: &AppHandle) -> Result<Option<Update>, String> {
    let mut builder = app.updater_builder().timeout(CHECK_TIMEOUT);
    if let Some(endpoints) = endpoint_override(endpoint_var().as_deref())? {
        log::info!("update check: using {ENDPOINT_ENV}={}", endpoints[0]);
        builder = builder.endpoints(endpoints).map_err(|e| e.to_string())?;
    }
    let updater = builder.build().map_err(|e| e.to_string())?;
    updater.check().await.map_err(|e| e.to_string())
}

async fn download(app: &AppHandle, mut update: Update, manual: bool) {
    // The check's timeout doesn't carry over; without one a stalled download would never end.
    update.timeout = Some(DOWNLOAD_TIMEOUT);
    let mut received: u64 = 0;
    let result = update
        .download(
            |chunk, total| {
                received += chunk as u64;
                advance(app, Step::Progress { received, total });
            },
            || {},
        )
        .await
        .map_err(|e| {
            use tauri_plugin_updater::Error::{Base64, Minisign, SignatureUtf8};
            (matches!(e, Minisign(_) | Base64(_) | SignatureUtf8(_)), e.to_string())
        })
        .and_then(|bytes| {
            // The plugin verified the minisign signature against tauri.conf.json's pubkey before returning.
            let dir = bundles_dir(app).ok_or((false, "no cache folder".to_string()))?;
            save_bundle(&dir, &update.version, &bytes).map_err(|e| (false, format!("couldn't save it: {e}")))
        });
    match result {
        Ok(path) => {
            log::info!("update: Perch {} downloaded and verified to {}", update.version, path.display());
            keep_ready(app, update, path);
        }
        Err((bad_signature, e)) => {
            if bad_signature {
                log::error!("update: Perch {} failed signature verification: {e}", update.version);
            } else {
                log::warn!("update: downloading Perch {} failed: {e}", update.version);
            }
            record_failure(app);
            advance(app, Step::DownloadFailed { manual, bad_signature });
        }
    }
}

/// Makes a freshly downloaded update the ready one and deletes any older bundle.
fn keep_ready(app: &AppHandle, update: Update, path: PathBuf) {
    let updates = app.state::<Updates>();
    if updates.installing.load(Ordering::SeqCst) {
        // A restart for the previous update is under way; this one is fetched again next launch.
        let _ = std::fs::remove_file(&path);
        return;
    }
    let (version, notes) = (update.version.clone(), update.body.as_deref().map(|n| n.chars().take(NOTES_MAX_CHARS).collect()));
    *lock(&updates.ready) = Some(ReadyUpdate { update, path: path.clone() });
    if let Some(dir) = path.parent() {
        remove_bundles(dir, Some(&path));
    }
    advance(app, Step::Downloaded { version, notes });
}

/// Installs the downloaded update and restarts Perch. Refused while an Ask run is active.
///
/// Windows: the plugin runs the NSIS installer with `/P /UPDATE /R` and exits this process;
/// the installer relaunches Perch when it's done (`/R`). macOS/Linux: the bundle or AppImage
/// is replaced in place, then Perch restarts itself.
pub fn install(app: &AppHandle) -> Result<(), String> {
    let updates = app.state::<Updates>();
    // Claim the install before looking for Asks; `ask` checks the claim under the same lock it reserves with.
    if updates.installing.swap(true, Ordering::SeqCst) {
        return Err(RESTARTING.into());
    }
    let result = install_claimed(app, &updates);
    if result.is_err() {
        updates.installing.store(false, Ordering::SeqCst);
    }
    result
}

fn install_claimed(app: &AppHandle, updates: &Updates) -> Result<(), String> {
    let asks_running = !lock(&app.state::<AppState>().running).is_empty();
    if let Some(reason) = install_refusal(&status(app), asks_running) {
        return Err(reason.into());
    }
    let ready = lock(&updates.ready).take().ok_or(NOT_READY)?;
    let bytes = match std::fs::read(&ready.path) {
        Ok(bytes) => bytes,
        Err(e) => {
            log::error!("update: couldn't read {}: {e}", ready.path.display());
            *lock(&updates.ready) = Some(ready);
            return Err(format!("Couldn't read the downloaded update: {e}"));
        }
    };
    log::info!("update: installing Perch {} and restarting", ready.update.version);
    match ready.update.install(&bytes) {
        Ok(()) => {
            app.request_restart();
            Ok(())
        }
        Err(e) => {
            log::error!("update: installing Perch {} failed: {e}", ready.update.version);
            *lock(&updates.ready) = Some(ready);
            Err(format!("Couldn't install the update: {e}"))
        }
    }
}

/// For `ask`, called under the `running` lock: no new Ask once a restart to update has begun.
pub fn ensure_not_installing(app: &AppHandle) -> Result<(), String> {
    if app.state::<Updates>().installing.load(Ordering::SeqCst) {
        Err(RESTARTING.into())
    } else {
        Ok(())
    }
}

// ---- Commands (registered in lib.rs) ----

/// Manual "Check for updates": returns once the check is done; a found update keeps downloading.
#[tauri::command]
pub async fn check_for_update(app: AppHandle) -> UpdateStatus {
    check(&app, true).await
}

#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<(), String> {
    install(&app)
}

#[tauri::command]
pub fn set_auto_update(app: AppHandle, enabled: bool) -> Snapshot {
    let s = app.state::<AppState>();
    lock(&s.config).auto_update = enabled;
    s.save_config();
    state::emit_snapshot(&app);
    state::snapshot(&app)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const HOUR: i64 = 60 * 60 * 1000;
    const NOW: i64 = 1_800_000_000_000;
    const NO_FAILURES: Backoff = Backoff { failures: 0, last_failure: None };

    fn st(state: UpdateState) -> UpdateStatus {
        UpdateStatus::with_state(state)
    }

    fn found(v: &str) -> Step {
        Step::Found { version: v.into(), notes: Some("Fixes".into()) }
    }

    fn downloaded(v: &str) -> Step {
        Step::Downloaded { version: v.into(), notes: Some("Fixes".into()) }
    }

    fn ready(v: &str) -> UpdateStatus {
        st(UpdateState::Checking).apply(found(v)).apply(downloaded(v))
    }

    fn failed_times(n: u32, last: i64) -> Backoff {
        (0..n).fold(NO_FAILURES, |b, _| b.failed(last))
    }

    #[test]
    fn launch_check_runs_whenever_auto_update_is_on() {
        assert!(check_due(true, true, None, NO_FAILURES, NOW));
        assert!(check_due(true, true, Some(NOW - HOUR), NO_FAILURES, NOW));
        assert!(check_due(true, true, Some(NOW - HOUR), failed_times(3, NOW - 60_000), NOW));
        assert!(!check_due(false, true, None, NO_FAILURES, NOW));
    }

    #[test]
    fn later_checks_wait_a_day_since_the_last_successful_one() {
        assert!(check_due(true, false, None, NO_FAILURES, NOW));
        assert!(!check_due(true, false, Some(NOW - 23 * HOUR), NO_FAILURES, NOW));
        assert!(check_due(true, false, Some(NOW - 24 * HOUR), NO_FAILURES, NOW));
        assert!(check_due(true, false, Some(NOW - 30 * 24 * HOUR), NO_FAILURES, NOW));
        assert!(!check_due(false, false, Some(NOW - 30 * 24 * HOUR), NO_FAILURES, NOW));
        assert!(!check_due(false, false, None, NO_FAILURES, NOW));
    }

    #[test]
    fn a_last_check_in_the_future_counts_as_due() {
        // The clock was set back after the last check.
        assert!(check_due(true, false, Some(NOW + HOUR), NO_FAILURES, NOW));
        assert!(check_due(true, false, Some(NOW - HOUR), failed_times(1, NOW + HOUR), NOW));
    }

    #[test]
    fn failed_checks_retry_after_an_hour_then_back_off_to_a_day() {
        let last_ok = Some(NOW - 3 * 24 * HOUR);
        let once = failed_times(1, NOW);
        assert!(!check_due(true, false, last_ok, once, NOW + 59 * 60_000));
        assert!(check_due(true, false, last_ok, once, NOW + HOUR));
        let twice = failed_times(2, NOW);
        assert!(!check_due(true, false, last_ok, twice, NOW + HOUR));
        assert!(check_due(true, false, last_ok, twice, NOW + 2 * HOUR));
        assert_eq!(failed_times(3, NOW).retry_delay_ms(), 4 * HOUR);
        assert_eq!(failed_times(6, NOW).retry_delay_ms(), CHECK_EVERY_MS);
        assert_eq!(failed_times(500, NOW).retry_delay_ms(), CHECK_EVERY_MS);
        assert!(!check_due(false, false, last_ok, once, NOW + 2 * HOUR));
    }

    #[test]
    fn backoff_counts_consecutive_failures() {
        let b = NO_FAILURES.failed(NOW).failed(NOW + HOUR);
        assert_eq!(b, Backoff { failures: 2, last_failure: Some(NOW + HOUR) });
        assert_eq!(Backoff { failures: u32::MAX, last_failure: None }.failed(NOW).failures, u32::MAX);
    }

    #[test]
    fn endpoint_default_unless_the_env_var_is_set() {
        assert_eq!(endpoint_override(None), Ok(None));
        assert_eq!(endpoint_override(Some("")), Ok(None));
        assert_eq!(endpoint_override(Some("   ")), Ok(None));
        let rc = "https://github.com/yeetstick/perch/releases/download/v1.0.0-rc.2/latest.json";
        assert_eq!(endpoint_override(Some(rc)), Ok(Some(vec![Url::parse(rc).unwrap()])));
        assert_eq!(endpoint_override(Some(&format!("  {rc}\n"))), Ok(Some(vec![Url::parse(rc).unwrap()])));
        let err = endpoint_override(Some("not a url")).unwrap_err();
        assert!(err.contains(ENDPOINT_ENV), "{err}");
    }

    #[test]
    fn dev_builds_only_update_from_an_override() {
        assert!(available(false, false));
        assert!(available(false, true));
        assert!(available(true, true));
        assert!(!available(true, false));
    }

    #[test]
    fn bundled_config_points_at_the_latest_release_and_signs_bundles() {
        let conf: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let updater = &conf["plugins"]["updater"];
        assert_eq!(
            updater["endpoints"],
            json!(["https://github.com/yeetstick/perch/releases/latest/download/latest.json"])
        );
        assert!(updater["pubkey"].as_str().is_some_and(|k| k.len() > 100));
        assert_eq!(updater["windows"]["installMode"], json!("passive"));
        assert_eq!(conf["bundle"]["createUpdaterArtifacts"], json!(true));
    }

    #[test]
    fn serializes_camel_case_without_unset_fields() {
        assert_eq!(serde_json::to_value(st(UpdateState::Idle)).unwrap(), json!({ "state": "idle" }));
        let s = st(UpdateState::Checking).apply(found("1.0.1")).apply(Step::Progress { received: 5, total: Some(10) });
        assert_eq!(
            serde_json::to_value(&s).unwrap(),
            json!({ "state": "downloading", "version": "1.0.1", "notes": "Fixes", "progress": 50 })
        );
    }

    #[test]
    fn a_check_that_finds_nothing_returns_to_idle() {
        let s = st(UpdateState::Idle).apply(Step::CheckStarted);
        assert_eq!(s.state, UpdateState::Checking);
        assert_eq!(s.apply(Step::UpToDate), st(UpdateState::Idle));
    }

    #[test]
    fn a_found_update_downloads_then_is_ready() {
        let s = st(UpdateState::Idle).apply(Step::CheckStarted).apply(found("1.0.1"));
        assert_eq!((s.state, s.version.as_deref()), (UpdateState::Available, Some("1.0.1")));
        let s = s.apply(Step::Progress { received: 0, total: None });
        assert_eq!((s.state, s.progress), (UpdateState::Downloading, None));
        let s = s.apply(Step::Progress { received: 250, total: Some(1000) });
        assert_eq!((s.state, s.progress, s.version.as_deref()), (UpdateState::Downloading, Some(25), Some("1.0.1")));
        let s = s.apply(downloaded("1.0.1"));
        assert_eq!(
            s,
            UpdateStatus {
                state: UpdateState::Ready,
                version: Some("1.0.1".into()),
                notes: Some("Fixes".into()),
                progress: None,
                error: None,
            }
        );
    }

    #[test]
    fn background_failures_are_quiet_and_manual_ones_say_so() {
        let checking = st(UpdateState::Idle).apply(Step::CheckStarted);
        assert_eq!(checking.apply(Step::CheckFailed { manual: false }), st(UpdateState::Idle));
        let e = checking.apply(Step::CheckFailed { manual: true });
        assert_eq!((e.state, e.error.as_deref()), (UpdateState::Error, Some(CHECK_FAILED)));

        let downloading = checking.apply(found("1.0.1")).apply(Step::Progress { received: 1, total: Some(2) });
        let quiet = downloading.apply(Step::DownloadFailed { manual: false, bad_signature: false });
        assert_eq!(quiet, st(UpdateState::Idle));
        let e = downloading.apply(Step::DownloadFailed { manual: true, bad_signature: false });
        assert_eq!((e.state, e.error.as_deref(), e.version), (UpdateState::Error, Some(DOWNLOAD_FAILED), None));
    }

    #[test]
    fn a_bad_signature_is_always_reported() {
        let downloading = st(UpdateState::Available).apply(Step::Progress { received: 1, total: None });
        for manual in [false, true] {
            let e = downloading.apply(Step::DownloadFailed { manual, bad_signature: true });
            assert_eq!((e.state, e.error.as_deref()), (UpdateState::Error, Some(BAD_SIGNATURE)));
        }
    }

    #[test]
    fn a_check_in_progress_is_not_restarted() {
        for state in [UpdateState::Checking, UpdateState::Available, UpdateState::Downloading] {
            let s = st(state).apply(found("1.0.1"));
            assert_eq!(s.apply(Step::CheckStarted), s, "{state:?}");
        }
        let off = UpdateStatus::disabled(DEV_BUILD);
        assert_eq!(off.apply(Step::CheckStarted), off);
        // A failed or idle state checks again.
        let failed = st(UpdateState::Checking).apply(Step::CheckFailed { manual: true });
        assert_eq!(failed.apply(Step::CheckStarted), st(UpdateState::Checking));
    }

    #[test]
    fn a_ready_update_stays_offered_until_a_newer_one_is_downloaded() {
        let r = ready("1.0.1");
        // Checks keep running while an update is ready, without hiding the card.
        for step in [
            Step::CheckStarted,
            Step::UpToDate,
            found("1.0.2"),
            Step::Progress { received: 5, total: Some(10) },
            Step::CheckFailed { manual: true },
            Step::DownloadFailed { manual: true, bad_signature: false },
            Step::DownloadFailed { manual: false, bad_signature: true },
        ] {
            assert_eq!(r.apply(step.clone()), r, "{step:?}");
        }
        let newer = r.apply(Step::Downloaded { version: "1.0.2".into(), notes: None });
        assert_eq!(newer, UpdateStatus { version: Some("1.0.2".into()), ..st(UpdateState::Ready) });
    }

    #[test]
    fn only_a_newer_version_replaces_a_ready_one() {
        assert!(is_newer("1.0.2", "1.0.1"));
        assert!(is_newer("1.1.0", "1.0.9"));
        assert!(is_newer("1.0.0-rc.2", "1.0.0-rc.1"));
        assert!(is_newer("1.0.0", "1.0.0-rc.3"));
        assert!(!is_newer("1.0.1", "1.0.1"));
        assert!(!is_newer("1.0.0", "1.0.1"));
        assert!(!is_newer("garbage", "1.0.1"));
        assert!(!is_newer("1.0.2", "garbage"));
    }

    #[test]
    fn progress_is_a_whole_percent() {
        assert_eq!(percent(0, Some(1000)), Some(0));
        assert_eq!(percent(999, Some(1000)), Some(99));
        assert_eq!(percent(1000, Some(1000)), Some(100));
        assert_eq!(percent(2000, Some(1000)), Some(100));
        assert_eq!(percent(10, None), None);
        assert_eq!(percent(10, Some(0)), None);
        assert_eq!(percent(u64::MAX, Some(u64::MAX)), Some(100));
    }

    #[test]
    fn install_is_refused_during_an_ask_or_before_an_update_is_ready() {
        let r = ready("1.0.1");
        assert_eq!(install_refusal(&r, true), Some(ASK_RUNNING));
        assert_eq!(install_refusal(&r, false), None);
        for state in [UpdateState::Idle, UpdateState::Checking, UpdateState::Downloading, UpdateState::Error] {
            assert_eq!(install_refusal(&st(state), false), Some(NOT_READY), "{state:?}");
        }
        assert_eq!(ASK_RUNNING, "Finish or stop the running ask first.");
    }

    #[test]
    fn downloaded_bundles_are_saved_to_disk_and_read_back() {
        let dir = tempfile::tempdir().unwrap();
        let updates = dir.path().join("updates");
        let path = save_bundle(&updates, "1.0.1", b"installer bytes").unwrap();
        assert_eq!(path, updates.join("perch-1.0.1.update"));
        assert_eq!(std::fs::read(&path).unwrap(), b"installer bytes");
        // A version can't escape the folder.
        let odd = save_bundle(&updates, "1.0.0+../../x", b"x").unwrap();
        assert_eq!(odd.parent(), Some(updates.as_path()));
        assert_eq!(odd.file_name().unwrap(), "perch-1.0.0_.._.._x.update");
    }

    #[test]
    fn stale_bundles_are_removed_except_the_one_kept() {
        let dir = tempfile::tempdir().unwrap();
        let old = save_bundle(dir.path(), "1.0.1", b"old").unwrap();
        let new = save_bundle(dir.path(), "1.0.2", b"new").unwrap();
        std::fs::write(dir.path().join("other.txt"), "not ours").unwrap();
        remove_bundles(dir.path(), Some(&new));
        assert!(!old.exists());
        assert!(new.exists());
        assert!(dir.path().join("other.txt").exists());
        remove_bundles(dir.path(), None);
        assert!(!new.exists());
        // A missing folder is fine.
        remove_bundles(&dir.path().join("missing"), None);
    }
}
