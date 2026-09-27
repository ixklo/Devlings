//! In-app updates (M1). Checks the signed `latest.json` on GitHub Releases with
//! tauri-plugin-updater, downloads an update in the background, and installs it
//! when the user clicks Restart. Everything runs in Rust; the webview has no
//! updater permissions and only sees `Snapshot.update`.

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
const LAUNCH_DELAY: Duration = Duration::from_secs(30);
const POLL_EVERY: Duration = Duration::from_secs(10 * 60);
const CHECK_TIMEOUT: Duration = Duration::from_secs(30);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(15 * 60);
const NOTES_MAX_CHARS: usize = 2000;

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
    Downloaded,
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

    /// True when a check must not start: one is running, an update is on its way or waiting, or updates are off.
    pub fn is_busy(&self) -> bool {
        use UpdateState::*;
        matches!(self.state, Checking | Available | Downloading | Ready | Disabled)
    }

    pub fn apply(&self, step: Step) -> UpdateStatus {
        use UpdateState::*;
        match step {
            Step::CheckStarted if self.is_busy() => self.clone(),
            Step::CheckStarted => Self::with_state(Checking),
            Step::UpToDate => Self::with_state(Idle),
            Step::Found { version, notes } => {
                Self { version: Some(version), notes, ..Self::with_state(Available) }
            }
            Step::Progress { received, total } => Self {
                state: Downloading,
                progress: percent(received, total),
                error: None,
                ..self.clone()
            },
            Step::Downloaded => Self { state: Ready, progress: None, error: None, ..self.clone() },
            Step::DownloadFailed { bad_signature: true, .. } => Self::failed(BAD_SIGNATURE),
            Step::CheckFailed { manual: true } => Self::failed(CHECK_FAILED),
            Step::DownloadFailed { manual: true, .. } => Self::failed(DOWNLOAD_FAILED),
            Step::CheckFailed { manual: false } | Step::DownloadFailed { manual: false, .. } => Self::with_state(Idle),
        }
    }

    fn failed(message: &str) -> Self {
        Self { error: Some(message.to_string()), ..Self::with_state(UpdateState::Error) }
    }
}

pub fn percent(received: u64, total: Option<u64>) -> Option<u8> {
    match total {
        Some(total) if total > 0 => Some((u128::from(received) * 100 / u128::from(total)).min(100) as u8),
        _ => None,
    }
}

/// Whether the scheduler should check now. `first` is the check ~30 s after launch.
pub fn check_due(auto: bool, first: bool, last: Option<i64>, now: i64) -> bool {
    if !auto {
        return false;
    }
    match last {
        _ if first => true,
        None => true,
        // A last check "in the future" means the clock moved back; check rather than wait it out.
        Some(at) => now < at || now - at >= CHECK_EVERY_MS,
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

// ---- Runtime ----

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn endpoint_var() -> Option<String> {
    std::env::var(ENDPOINT_ENV).ok()
}

/// Managed state: the status the UI sees, plus the downloaded update waiting for a restart.
pub struct Updates {
    status: Mutex<UpdateStatus>,
    ready: Mutex<Option<(Update, Vec<u8>)>>,
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
        Self { status: Mutex::new(status), ready: Mutex::new(None), installing: AtomicBool::new(false) }
    }
}

pub fn status(app: &AppHandle) -> UpdateStatus {
    lock(&app.state::<Updates>().status).clone()
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

/// Checks ~30 s after launch, then whenever the last check is a day old, while auto-update is on.
pub fn start(app: AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(LAUNCH_DELAY);
        let mut first = true;
        loop {
            let (auto, last) = {
                let s = app.state::<AppState>();
                let config = lock(&s.config);
                (config.auto_update, config.last_update_check)
            };
            if check_due(auto, first, last, now_ms()) {
                tauri::async_runtime::block_on(check(&app, false));
            }
            first = false;
            std::thread::sleep(POLL_EVERY);
        }
    });
}

/// Looks for an update and, when there is one, starts downloading it in the background.
/// Returns the status once the check itself is done. Background failures stay quiet (logged only).
pub async fn check(app: &AppHandle, manual: bool) -> UpdateStatus {
    {
        // Test and set under one lock, so a manual check and the scheduler never both start.
        let updates = app.state::<Updates>();
        let mut status = lock(&updates.status);
        if status.is_busy() {
            return status.clone();
        }
        *status = status.apply(Step::CheckStarted);
    }
    state::emit_snapshot(app);
    {
        let s = app.state::<AppState>();
        lock(&s.config).last_update_check = Some(now_ms());
        s.save_config();
    }
    match find(app).await {
        Ok(None) => {
            log::info!("update check: Perch is up to date");
            advance(app, Step::UpToDate)
        }
        Ok(Some(update)) => {
            log::info!("update check: Perch {} is available", update.version);
            let notes = update.body.as_deref().map(|n| n.chars().take(NOTES_MAX_CHARS).collect());
            let found = advance(app, Step::Found { version: update.version.clone(), notes });
            let handle = app.clone();
            tauri::async_runtime::spawn(async move { download(&handle, update, manual).await });
            found
        }
        Err(e) => {
            // Offline, a 404 (e.g. a latest release without latest.json) or a bad response.
            log::warn!("update check failed: {e}");
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
        .await;
    match result {
        Ok(bytes) => {
            // The plugin verified the minisign signature against tauri.conf.json's pubkey before returning.
            log::info!("update: Perch {} downloaded and verified ({} bytes)", update.version, bytes.len());
            *lock(&app.state::<Updates>().ready) = Some((update, bytes));
            advance(app, Step::Downloaded);
        }
        Err(e) => {
            use tauri_plugin_updater::Error::{Base64, Minisign, SignatureUtf8};
            let bad_signature = matches!(e, Minisign(_) | Base64(_) | SignatureUtf8(_));
            if bad_signature {
                log::error!("update: Perch {} failed signature verification: {e}", update.version);
            } else {
                log::warn!("update: downloading Perch {} failed: {e}", update.version);
            }
            advance(app, Step::DownloadFailed { manual, bad_signature });
        }
    }
}

/// Installs the downloaded update and restarts Perch. Refused while an Ask run is active.
///
/// Windows: the plugin runs the NSIS installer with `/P /UPDATE /R` and exits this process;
/// the installer relaunches Perch when it's done (`/R`). macOS/Linux: the bundle or AppImage
/// is replaced in place, then Perch restarts itself.
pub fn install(app: &AppHandle) -> Result<(), String> {
    let updates = app.state::<Updates>();
    // Claim the install before looking for Asks, so an Ask can't slip in between.
    if updates.installing.swap(true, Ordering::SeqCst) {
        return Err(RESTARTING.into());
    }
    let asks_running = !lock(&app.state::<AppState>().running).is_empty();
    let taken = match install_refusal(&status(app), asks_running) {
        Some(reason) => Err(reason),
        None => lock(&updates.ready).take().ok_or(NOT_READY),
    };
    let (update, bytes) = match taken {
        Ok(ready) => ready,
        Err(reason) => {
            updates.installing.store(false, Ordering::SeqCst);
            return Err(reason.into());
        }
    };
    log::info!("update: installing Perch {} and restarting", update.version);
    match update.install(&bytes) {
        Ok(()) => {
            app.request_restart();
            Ok(())
        }
        Err(e) => {
            log::error!("update: installing Perch {} failed: {e}", update.version);
            *lock(&updates.ready) = Some((update, bytes));
            updates.installing.store(false, Ordering::SeqCst);
            Err(format!("Couldn't install the update: {e}"))
        }
    }
}

/// For `ask`: no new Ask once a restart to update has begun.
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

    fn st(state: UpdateState) -> UpdateStatus {
        UpdateStatus::with_state(state)
    }

    fn found(v: &str) -> Step {
        Step::Found { version: v.into(), notes: Some("Fixes".into()) }
    }

    #[test]
    fn launch_check_runs_whenever_auto_update_is_on() {
        assert!(check_due(true, true, None, NOW));
        assert!(check_due(true, true, Some(NOW - HOUR), NOW));
        assert!(!check_due(false, true, None, NOW));
    }

    #[test]
    fn later_checks_wait_a_day_since_the_last_one() {
        assert!(check_due(true, false, None, NOW));
        assert!(!check_due(true, false, Some(NOW - 23 * HOUR), NOW));
        assert!(check_due(true, false, Some(NOW - 24 * HOUR), NOW));
        assert!(check_due(true, false, Some(NOW - 30 * 24 * HOUR), NOW));
        assert!(!check_due(false, false, Some(NOW - 30 * 24 * HOUR), NOW));
        assert!(!check_due(false, false, None, NOW));
    }

    #[test]
    fn a_last_check_in_the_future_counts_as_due() {
        // The clock was set back after the last check.
        assert!(check_due(true, false, Some(NOW + HOUR), NOW));
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
        let s = s.apply(Step::Downloaded);
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
    fn checks_do_not_restart_a_download_or_replace_a_ready_update() {
        for state in [UpdateState::Checking, UpdateState::Available, UpdateState::Downloading, UpdateState::Ready] {
            assert!(st(state).is_busy(), "{state:?}");
        }
        assert!(UpdateStatus::disabled(DEV_BUILD).is_busy());
        let ready = st(UpdateState::Checking).apply(found("1.0.1")).apply(Step::Downloaded);
        assert_eq!(ready.apply(Step::CheckStarted), ready);
        // A failed or idle state can check again.
        assert!(!st(UpdateState::Idle).is_busy());
        let failed = st(UpdateState::Checking).apply(Step::CheckFailed { manual: true });
        assert!(!failed.is_busy());
        assert_eq!(failed.apply(Step::CheckStarted), st(UpdateState::Checking));
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
        let ready = st(UpdateState::Checking).apply(found("1.0.1")).apply(Step::Downloaded);
        assert_eq!(install_refusal(&ready, true), Some(ASK_RUNNING));
        assert_eq!(install_refusal(&ready, false), None);
        for state in [UpdateState::Idle, UpdateState::Checking, UpdateState::Downloading, UpdateState::Error] {
            assert_eq!(install_refusal(&st(state), false), Some(NOT_READY), "{state:?}");
        }
        assert_eq!(ASK_RUNNING, "Finish or stop the running ask first.");
    }
}
