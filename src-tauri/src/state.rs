use std::{
    collections::{HashMap, HashSet},
    ffi::OsStr,
    io::{BufReader, Read},
    path::{Path, PathBuf},
    process::Child,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Mutex,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

use crate::{
    approvals::{self, PendingApproval, Registry, Resolved, Responder, WatchPolicy},
    events::{Kind, PetEvent, Source},
    hook_server::HookServer,
    hooks_installer::{self, HookStatus, HookTarget, Migration, RelayExe},
    locator::{self, Located},
    locks::lock,
    money_guard::AuthVerdict,
    normalize::{self, StreamItem},
    overlay::HitRect,
    pets::{self, Pet},
    runner::{self, KillReason, StdinWriter},
    shell,
    store::{self, Config, ProjectEntry, Projects},
    threads::{self, PetState, ThreadInfo, ThreadStatus, Threads},
};

/// What the bubbles show; the ticker compares it to catch changes caused only by time passing.
type ViewKey = (PetState, Vec<String>, Vec<String>);

fn temp_dir() -> String {
    std::env::temp_dir().to_string_lossy().to_string()
}

pub fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

pub struct AppState {
    pub data_dir: PathBuf,
    pub config: Mutex<Config>,
    pub projects: Mutex<Projects>,
    pub threads: Mutex<Threads>,
    pub focused_thread: Mutex<Option<String>>,
    pub claude: Mutex<Result<Located, String>>,
    pub auth: Mutex<Option<AuthVerdict>>,
    pub hooks_installed: AtomicBool,
    pub hook_server: Mutex<Option<HookServer>>,
    pub hook_server_error: Mutex<Option<String>>,
    pub running: Mutex<HashMap<String, u32>>,
    pub stop_requested: Mutex<HashSet<String>>,
    pub ask_sids: Mutex<HashMap<String, String>>,
    /// Permission requests waiting for an answer (design v1.0 D3-D5).
    pub approvals: Mutex<Registry>,
    pub last_view: Mutex<Option<ViewKey>>,
    pub pets: Mutex<Vec<Pet>>,
    /// Interactive rects of the pet window; None until the frontend first reports them.
    pub hit_regions: Mutex<Option<Vec<HitRect>>>,
    pub pet_visibility_gen: AtomicU64,
    /// Whether cards currently open below the sprite instead of above it (design D9). Runtime
    /// only, for the flip's hysteresis; not persisted, so a restart re-derives it from scratch.
    pub cards_below: AtomicBool,
}

impl AppState {
    pub fn load(data_dir: PathBuf) -> std::io::Result<Self> {
        std::fs::create_dir_all(&data_dir)?;
        let mut projects: Projects = store::load(&data_dir.join("projects.json"));
        projects.retain_outside(&temp_dir());
        let config = store::load::<Config>(&data_dir.join("config.json")).normalized();
        let mut approvals = Registry::default();
        approvals.set_watching(config.watch_approvals);
        Ok(Self {
            config: Mutex::new(config),
            projects: Mutex::new(projects),
            data_dir,
            threads: Mutex::new(Threads::default()),
            focused_thread: Mutex::new(None),
            claude: Mutex::new(Err("Checking for Claude Code…".to_string())),
            auth: Mutex::new(None),
            hooks_installed: AtomicBool::new(false),
            hook_server: Mutex::new(None),
            hook_server_error: Mutex::new(None),
            running: Mutex::new(HashMap::new()),
            stop_requested: Mutex::new(HashSet::new()),
            ask_sids: Mutex::new(HashMap::new()),
            approvals: Mutex::new(approvals),
            last_view: Mutex::new(None),
            pets: Mutex::new(Vec::new()),
            hit_regions: Mutex::new(None),
            pet_visibility_gen: AtomicU64::new(0),
            cards_below: AtomicBool::new(false),
        })
    }

    pub fn save_config(&self) {
        let _ = store::save(&self.data_dir.join("config.json"), &*lock(&self.config));
    }

    pub fn save_projects(&self) {
        let _ = store::save(&self.data_dir.join("projects.json"), &*lock(&self.projects));
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SetupStatus {
    pub claude_path: Option<String>,
    pub claude_version: Option<String>,
    pub claude_error: Option<String>,
    pub hooks_installed: bool,
    pub auth: Option<AuthVerdict>,
    pub hook_server_error: Option<String>,
    /// Something the user should do that doesn't block setup, e.g. "Move Perch to Applications…".
    pub setup_hint: Option<String>,
    pub needs_setup: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub config: Config,
    pub pet_state: PetState,
    pub threads: Vec<ThreadInfo>,
    pub projects: Vec<ProjectEntry>,
    pub running: Vec<String>,
    pub setup: SetupStatus,
    pub update: crate::updater::UpdateStatus,
    /// Permission requests the user can answer from the pet, oldest first.
    pub approvals: Vec<PendingApproval>,
}

fn setup_status(s: &AppState) -> SetupStatus {
    let claude = lock(&s.claude).clone();
    let hooks_declined = lock(&s.config).hooks_declined;
    let auth = lock(&s.auth).clone();
    let hooks_installed = s.hooks_installed.load(Ordering::SeqCst);
    let hook_server_error = lock(&s.hook_server_error).clone();
    let auth_refused = matches!(auth, Some(AuthVerdict::Refused { .. }));
    let needs_setup = claude.is_err()
        || hook_server_error.is_some()
        || auth_refused
        || (!hooks_installed && !hooks_declined);
    SetupStatus {
        claude_path: claude.as_ref().ok().map(|l| l.path.display().to_string()),
        claude_version: claude.as_ref().ok().map(|l| l.version.clone()),
        claude_error: claude.err(),
        hooks_installed,
        auth,
        hook_server_error,
        setup_hint: hooks_installer::relay_exe().ok().and_then(|r| r.setup_hint()),
        needs_setup,
    }
}

fn view_key(threads: &[ThreadInfo], pet_state: PetState, approvals: &[PendingApproval]) -> ViewKey {
    (
        pet_state,
        threads.iter().map(|t| t.session_id.clone()).collect(),
        approvals.iter().map(|a| a.id.clone()).collect(),
    )
}

pub fn snapshot(app: &AppHandle) -> Snapshot {
    let s = app.state::<AppState>();
    let setup = setup_status(&s);
    let threads = lock(&s.threads).visible(now_ms());
    let approvals = lock(&s.approvals).pending();
    let pet_state = threads::with_approvals(threads::pet_state(&threads, setup.needs_setup), !approvals.is_empty());
    let projects = lock(&s.projects).sorted();
    let running = lock(&s.running).keys().cloned().collect();
    let mut config = lock(&s.config).clone();
    // Report the pet that is actually shown, so a removed pet falls back to the default everywhere.
    if let Some(pet) = pets::resolve(&lock(&s.pets), &config.pet_id) {
        config.pet_id = pet.info.id.clone();
    }
    Snapshot { config, pet_state, threads, projects, running, setup, update: crate::updater::status(app), approvals }
}

/// Rescans every pet folder and caches the result. Also returns whether the shown pet changed.
fn scan_pets(app: &AppHandle) -> (Vec<Pet>, bool) {
    let s = app.state::<AppState>();
    let home = dirs::home_dir().unwrap_or_default();
    let roots = pets::roots(
        app.path().resource_dir().ok(),
        &s.data_dir,
        &pets::codex_home(std::env::var_os("CODEX_HOME"), &home),
    );
    let found = pets::discover(&roots);
    let wanted = lock(&s.config).pet_id.clone();
    let shown = |list: &[Pet]| pets::resolve(list, &wanted).map(|p| p.info.id.clone());
    let old = std::mem::replace(&mut *lock(&s.pets), found.clone());
    let changed = shown(&old) != shown(&found);
    (found, changed)
}

/// Rescans pets, and publishes a snapshot if that changed which pet is shown (e.g. its folder was deleted).
pub fn refresh_pets(app: &AppHandle) -> Vec<Pet> {
    let (found, changed) = scan_pets(app);
    if changed {
        emit_snapshot(app);
    }
    found
}

pub fn emit_snapshot(app: &AppHandle) {
    let snap = snapshot(app);
    *lock(&app.state::<AppState>().last_view) = Some(view_key(&snap.threads, snap.pet_state, &snap.approvals));
    let _ = app.emit("snapshot", &snap);
    shell::update_tray_tooltip(app, &snap);
}

pub fn handle_event(app: &AppHandle, ev: PetEvent) {
    let s = app.state::<AppState>();
    // A session with a request still waiting stays in "needs input" through its other activity (D5).
    let waiting = lock(&s.approvals).waiting(&ev.session_id);
    let status = lock(&s.threads).get(&ev.session_id).map(|t| t.status);
    let ev = approvals::keep_needs_input(ev, status, waiting);
    let applied = lock(&s.threads).apply(&ev);
    let _ = app.emit("pet-event", &ev);
    if let Some(status) = applied.alert {
        maybe_notify(app, &ev, status);
    }
    // Reply deltas arrive many times a second; only the first of a reply changes what the bubbles show.
    if ev.kind != Kind::ReplyDelta || applied.changed {
        emit_snapshot(app);
    }
}

/// Sends an Ask's untrusted-folder notice to its mini chat (design v1.0 D6). It rides on `pet-event` like the
/// run's own messages, but changes no thread, so it skips the thread table and the snapshot.
pub fn emit_ask_notice(app: &AppHandle, ev: &PetEvent) {
    let _ = app.emit("pet-event", ev);
}

fn maybe_notify(app: &AppHandle, ev: &PetEvent, status: ThreadStatus) {
    let s = app.state::<AppState>();
    let (enabled, pet) = {
        let c = lock(&s.config);
        (c.notifications, c.pet_name.clone())
    };
    let focused = lock(&s.focused_thread).as_deref() == Some(ev.session_id.as_str());
    if !enabled || focused {
        return;
    }
    let short = |t: &str| threads::excerpt(t).map(|x| x.chars().take(120).collect::<String>());
    let body = match status {
        ThreadStatus::NeedsInput => "Needs your approval".to_string(),
        ThreadStatus::Ready => ev.text.as_deref().and_then(short).unwrap_or_else(|| "Done".to_string()),
        ThreadStatus::Blocked => ev
            .text
            .as_deref()
            .and_then(short)
            .or_else(|| ev.label.clone())
            .unwrap_or_else(|| "Something went wrong".to_string()),
        ThreadStatus::Running | ThreadStatus::Idle => return,
    };
    let project = lock(&s.threads)
        .get(&ev.session_id)
        .map(|t| t.project_name.clone())
        .unwrap_or_else(|| store::project_name(&ev.project));
    let _ = app.notification().builder().title(format!("{pet} · {project}")).body(body).show();
}

pub fn on_hook_body(app: &AppHandle, body: Value) {
    let s = app.state::<AppState>();
    let sid = body.get("session_id").and_then(Value::as_str).unwrap_or("").to_string();
    let ask_project = lock(&s.ask_sids).get(&sid).cloned();
    if let Some(project) = ask_project {
        if let Some(tp) = body.get("transcript_path").and_then(Value::as_str) {
            let changed = lock(&s.projects).set_transcript_path(&project, tp);
            if changed {
                s.save_projects();
            }
        }
        return;
    }
    let now = now_ms();
    // A tool that ran, a denial, a new prompt or the end of a turn resolves waiting requests (design v1.0 D5).
    let cleared = approvals::apply_hook_event(&mut lock(&s.approvals), &body);
    if let Some(ev) = normalize::from_hook(&body, now) {
        // Sessions in scratch folders (e.g. other agents' temp dirs) still show in Activity but don't become projects.
        let is_project = !ev.project.is_empty() && !store::is_under(&ev.project, &temp_dir());
        if is_project && lock(&s.projects).touch(&ev.project, ev.at) {
            s.save_projects();
        }
        handle_event(app, ev);
    }
    let status = lock(&s.threads).get(&sid).map(|t| t.status);
    let resume = approvals::resume_after(&lock(&s.approvals), status, &body, now);
    match resume {
        Some(ev) => handle_event(app, ev),
        None if cleared > 0 => emit_snapshot(app),
        None => {}
    }
}

/// A PermissionRequest hook, on its worker: the answer to send back (design v1.0 D3). May block while held.
fn on_permission_body(app: &AppHandle, body: Value) -> String {
    let s = app.state::<AppState>();
    let sid = body.get("session_id").and_then(Value::as_str).unwrap_or("");
    let own_ask = lock(&s.ask_sids).contains_key(sid);
    let hold = Duration::from_secs(lock(&s.config).approval_hold_secs);
    let policy = WatchPolicy { hold, own_ask };
    approvals::on_watch_request(&s.approvals, &body, policy, now_ms(), &|| emit_snapshot(app))
}

/// Puts threads back to work once none of their session's requests wait any more, then publishes.
pub fn after_resolved(app: &AppHandle, resolved: Vec<Resolved>) {
    let s = app.state::<AppState>();
    for r in resolved {
        let waiting = lock(&s.approvals).waiting(&r.session_id);
        let needs_input = lock(&s.threads).get(&r.session_id).map(|t| t.status) == Some(ThreadStatus::NeedsInput);
        if !waiting && needs_input {
            handle_event(app, r.event(now_ms()));
        }
    }
    emit_snapshot(app);
}

/// Perch is quitting: nothing may keep waiting on it. Held hooks get "no decision" and Ask requests are denied.
pub fn shutdown(app: &AppHandle) {
    let Some(s) = app.try_state::<AppState>() else { return };
    let released = lock(&s.approvals).release_all(approvals::QUIT_MESSAGE);
    if released > 0 {
        log::info!("Answered {released} pending permission request(s) before quitting");
        // Let the hook workers and stdin writers send those answers before the process ends.
        std::thread::sleep(Duration::from_millis(300));
    }
}

pub fn start_hook_server(app: &AppHandle) {
    let s = app.state::<AppState>();
    if let Some(old) = lock(&s.hook_server).take() {
        old.stop();
    }
    let (port, token) = {
        let c = lock(&s.config);
        (c.hook_port, c.hook_token.clone())
    };
    let (Some(port), Some(token)) = (port, token) else { return };
    let handle = app.clone();
    // Already answered; runs on the hook server's single processing thread, in arrival order.
    let on_event = move |body: Value| on_hook_body(&handle, body);
    // Runs on the request's worker and its answer is the response; a watched request may be held here.
    let perm = app.clone();
    let on_permission = move |body: Value| on_permission_body(&perm, body);
    match HookServer::start(port, token, on_event, on_permission) {
        Ok(server) => {
            let bound_port = server.port;
            log::info!("Hook server listening on 127.0.0.1:{bound_port}");
            *lock(&s.hook_server) = Some(server);
            *lock(&s.hook_server_error) = None;
            if lock(&s.config).hook_port != Some(bound_port) {
                lock(&s.config).hook_port = Some(bound_port);
                s.save_config();
            }
        }
        Err(e) => {
            log::error!("Hook server didn't start: {e}");
            *lock(&s.hook_server_error) = Some(format!("{e}. Use \"Move to a new port\" in Settings."));
        }
    }
}

/// The installed hooks compared with this build's, or None when Perch has no port and token yet.
pub fn hook_status(app: &AppHandle) -> Option<Result<HookStatus, String>> {
    let s = app.state::<AppState>();
    let (port, token) = {
        let c = lock(&s.config);
        (c.hook_port, c.hook_token.clone())
    };
    let (port, token) = (port?, token?);
    Some(hooks_installer::relay_exe().and_then(|relay| {
        let settings = hooks_installer::read_settings(&hooks_installer::settings_path())?;
        Ok(hooks_installer::status(&settings, HookTarget { port, token: &token, exe: relay.usable() }))
    }))
}

/// Brings installed hooks up to date with this build, once, when they differ (design v1.0 §3).
fn migrate_hooks(app: &AppHandle) {
    let s = app.state::<AppState>();
    let (port, token) = {
        let c = lock(&s.config);
        (c.hook_port, c.hook_token.clone())
    };
    let (Some(port), Some(token)) = (port, token) else { return };
    let relay = match hooks_installer::relay_exe() {
        Ok(RelayExe::Ephemeral(exe)) => {
            // Writing this path would leave hooks pointing at a program that is gone after this run.
            log::warn!("Hook migration skipped: Perch is running from a temporary location ({})", exe.display());
            return;
        }
        Ok(relay) => relay,
        Err(e) => {
            log::error!("Hook migration skipped: {e}");
            return;
        }
    };
    if let RelayExe::Unsafe(exe) = &relay {
        log::warn!("Perch's path has shell characters, so Stop, StopFailure and SessionEnd use HTTP ({})", exe.display());
    }
    let path = hooks_installer::settings_path();
    let target = HookTarget { port, token: &token, exe: relay.usable() };
    match hooks_installer::migrate_file(&path, target, now_ms() / 1000) {
        Ok(Migration::Migrated) => log::info!("Hooks updated to this version's entries in {}", path.display()),
        Ok(Migration::UpToDate) => log::info!("Hooks are up to date"),
        Ok(Migration::NotInstalled) => log::info!("Hooks aren't installed"),
        Err(e) => log::error!("Hook migration failed: {e}"),
    }
}

pub fn refresh_hooks_installed(app: &AppHandle) {
    let installed = matches!(hook_status(app), Some(Ok(HookStatus::Current)));
    app.state::<AppState>().hooks_installed.store(installed, Ordering::SeqCst);
}

/// What a setup recheck found: a pure function (candidates/version/auth are all injected) so a
/// test can prove it never reuses a cached verdict, without any Tauri app to construct.
pub struct RecheckOutcome {
    pub located: Result<Located, String>,
    pub auth: Option<AuthVerdict>,
}

/// Locates Claude Code and checks its login fresh, every time it's called. `auth_status_of` is
/// only ever invoked with the binary this same call just located, so Recheck (and any other
/// caller) can never show a login verdict left over from a previous check or a previous binary.
pub fn compute_recheck(
    override_path: Option<&Path>,
    path_env: Option<&OsStr>,
    home: &Path,
    version_of: impl Fn(&Path) -> Option<String>,
    auth_status_of: impl Fn(&Path) -> AuthVerdict,
) -> RecheckOutcome {
    let located = locator::relocate(override_path, path_env, home, version_of);
    let auth = located.as_ref().ok().map(|l| auth_status_of(&l.path));
    RecheckOutcome { located, auth }
}

pub fn recheck_setup(app: &AppHandle) {
    let s = app.state::<AppState>();
    let override_path = lock(&s.config).claude_path.clone();
    let home = dirs::home_dir().unwrap_or_default();
    let path_env = std::env::var_os("PATH");
    let outcome = compute_recheck(override_path.as_deref().map(Path::new), path_env.as_deref(), &home, runner::version_of, runner::auth_status);
    match &outcome.located {
        Ok(l) => {
            let via = locator::describe_source(&l.path, override_path.as_deref().map(Path::new), path_env.as_deref(), &home);
            log::info!("Claude Code {} at {} (found via {via})", l.version, l.path.display());
        }
        Err(e) => log::warn!("Claude Code not usable: {e}"),
    }
    match &outcome.auth {
        Some(AuthVerdict::Allowed { subscription }) => log::info!("Claude Code login: {subscription} subscription"),
        Some(AuthVerdict::Refused { reason }) => log::warn!("Claude Code login refused: {reason}"),
        None => {}
    }
    *lock(&s.claude) = outcome.located;
    *lock(&s.auth) = outcome.auth;
    refresh_hooks_installed(app);
}

/// Re-locates Claude Code if the remembered path no longer runs (design gap G3.5): checked before
/// every Ask spawn, and on a plain setup recheck. A stale user-chosen path is skipped like any
/// missing candidate (see `locator::relocate`), so this falls back to auto-detection on its own.
/// Returns the binary to run, refreshing `s.claude`/`s.auth` first if it had to re-locate.
pub fn ensure_claude_located(app: &AppHandle) -> Result<PathBuf, String> {
    let located = lock(&app.state::<AppState>().claude).clone();
    let stale = match &located {
        Ok(l) => locator::is_stale(&l.path, runner::version_of),
        Err(_) => true,
    };
    if !stale {
        return located.map(|l| l.path);
    }
    log::info!("Claude Code's remembered path is stale; re-locating");
    recheck_setup(app);
    lock(&app.state::<AppState>().claude).clone().map(|l| l.path)
}

pub fn boot(app: AppHandle) {
    std::thread::spawn(move || {
        scan_pets(&app);
        migrate_hooks(&app);
        recheck_setup(&app);
        start_hook_server(&app);
        emit_snapshot(&app);
        let onboarded = lock(&app.state::<AppState>().config).onboarded;
        if !onboarded {
            // Window calls from this thread are queued; on the main thread they run in order, so open_settings can see and undo a minimized start.
            let handle = app.clone();
            let _ = app.run_on_main_thread(move || {
                let _ = shell::open_settings(&handle, shell::SettingsView::Onboarding);
            });
        }
        loop {
            std::thread::sleep(Duration::from_secs(1));
            tick(&app);
        }
    });
}

fn tick(app: &AppHandle) {
    let s = app.state::<AppState>();
    let now = now_ms();
    let overdue = {
        let mut a = lock(&s.approvals);
        a.prune(now);
        a.deny_overdue_asks(now)
    };
    if !overdue.is_empty() {
        log::info!("Denied {} Ask permission request(s) nobody answered", overdue.len());
        after_resolved(app, overdue);
    }
    let threads = {
        let mut t = lock(&s.threads);
        t.prune(now);
        t.visible(now)
    };
    let approvals = lock(&s.approvals).pending();
    let pet_state = threads::with_approvals(threads::pet_state(&threads, setup_status(&s).needs_setup), !approvals.is_empty());
    let view = view_key(&threads, pet_state, &approvals);
    let changed = lock(&s.last_view).as_ref() != Some(&view);
    if changed {
        emit_snapshot(app);
    }
}

fn ask_event(sid: &str, project: &str, kind: Kind, label: &str, text: Option<String>) -> PetEvent {
    PetEvent {
        session_id: sid.to_string(),
        project: project.to_string(),
        source: Source::Ask,
        kind,
        label: Some(label.to_string()),
        text,
        at: now_ms(),
    }
}

fn take_stop_request(s: &AppState, project: &str) -> bool {
    let mut set = lock(&s.stop_requested);
    let hit = set.iter().find(|p| store::same_path(p, project)).cloned();
    if let Some(h) = &hit {
        set.remove(h);
    }
    hit.is_some()
}

pub fn run_ask(app: AppHandle, project: String, mut child: Child, stdin: StdinWriter) {
    let s = app.state::<AppState>();
    let pid = child.id();
    let stderr_thread = child.stderr.take().map(|mut err| {
        std::thread::spawn(move || {
            let mut text = String::new();
            let _ = err.read_to_string(&mut text);
            text
        })
    });
    let Some(stdout) = child.stdout.take() else {
        log::error!("Ask in {project}: Claude Code's output wasn't captured");
        runner::kill_tree(pid);
        let _ = child.wait();
        lock(&s.running).retain(|p, _| !store::same_path(p, &project));
        emit_snapshot(&app);
        return;
    };
    let mut session_id = format!("ask:{project}");
    let mut finished = false;

    let kill = runner::pump(BufReader::new(stdout), &project, now_ms, &stdin, |item| match item {
        StreamItem::Init { session_id: sid, .. } => {
            session_id = sid.clone();
            lock(&s.ask_sids).insert(sid.clone(), project.clone());
            if let Some(p) = lock(&s.projects).get_mut(&project) {
                p.ask_session_id = Some(sid);
            }
            s.save_projects();
        }
        StreamItem::Pet(ev) => {
            if matches!(ev.kind, Kind::Done | Kind::Failed) {
                finished = true;
            }
            let started = ev.kind == Kind::Started;
            let sid = ev.session_id.clone();
            handle_event(&app, ev);
            if started {
                // A headless run reports nothing until its first reply token or tool call; show the thread as working now.
                handle_event(&app, ask_event(&sid, &project, Kind::Prompt, "Thinking…", None));
            }
        }
        StreamItem::LimitRejected { resets_at } => {
            finished = true;
            let text = normalize::resets_text(resets_at, now_ms());
            handle_event(&app, ask_event(&session_id, &project, Kind::Failed, "Plan limit reached", Some(text)));
        }
        StreamItem::CanUseTool { request_id, request } => {
            // Claude Code asks before running a tool (design v1.0 D4); held until answered in Perch.
            let req = approvals::Request::from_can_use_tool(&session_id, &project, &request);
            if let Some(dialog) = approvals::dialog_kind(&req.tool_name) {
                // A question or a plan is never a plain Allow: turn it down with a message Claude can act on.
                log::info!("Ask in {project}: {} answered with a note", approvals::log_safe(&req.tool_name));
                stdin.send(approvals::host_deny(&request_id, dialog.ask_deny_message()));
                handle_event(&app, ask_event(&session_id, &project, Kind::Blocked, dialog.chat_note(), None));
                return;
            }
            log::info!("Ask in {project} asks to use {}", approvals::log_safe(&req.tool_name));
            let responder = Responder::Ask { stdin: stdin.clone(), request_id };
            lock(&s.approvals).add(req, Source::Ask, Some(responder), now_ms(), None);
            handle_event(&app, ask_event(&session_id, &project, Kind::NeedsYou, normalize::NEEDS_APPROVAL, None));
        }
        StreamItem::ControlCancel { request_id } => {
            let withdrawn = lock(&s.approvals).cancel_ask(&project, &request_id);
            if let Some(r) = withdrawn {
                after_resolved(&app, vec![r]);
            }
        }
        // The pump refuses these itself.
        StreamItem::ControlRequest { .. } => {}
        StreamItem::Overage { .. } => {}
    });

    if let Some(reason) = &kill {
        log::warn!("Ask in {project} stopped by the money guard: {reason:?}");
        runner::kill_tree(pid);
        finished = true;
        let (label, text) = match reason {
            KillReason::NotSubscription(msg) => ("Stopped: not using your subscription", msg.clone()),
            KillReason::Overage { resets_at } => (
                "Stopped before using paid credits",
                format!(
                    "Your plan limit is used up and this would bill usage credits. {}",
                    normalize::resets_text(*resets_at, now_ms())
                ),
            ),
        };
        handle_event(&app, ask_event(&session_id, &project, Kind::Failed, label, Some(text)));
    }

    // Nothing can answer this run any more; a closed stdin lets Claude Code exit on its own.
    stdin.close();
    let status = child.wait();
    let stderr_text = stderr_thread.and_then(|h| h.join().ok()).unwrap_or_default();
    match &status {
        Ok(st) => log::info!("Ask in {project} exited ({st})"),
        Err(e) => log::warn!("Ask in {project}: couldn't wait for Claude Code: {e}"),
    }
    if take_stop_request(&s, &project) {
        log::info!("Ask in {project} stopped by the user");
        handle_event(&app, ask_event(&session_id, &project, Kind::Ended, "Stopped", None));
    } else if !finished {
        let code = status.ok().and_then(|st| st.code()).map(|c| c.to_string()).unwrap_or_else(|| "?".into());
        let err = stderr_text.trim();
        let text = if err.is_empty() {
            format!("Claude Code exited unexpectedly (code {code}).")
        } else {
            err.chars().take(400).collect()
        };
        log::warn!("Ask in {project} ended without a result: {text}");
        handle_event(&app, ask_event(&session_id, &project, Kind::Failed, "Something went wrong", Some(text)));
    }
    // Nothing can answer this run's requests any more.
    lock(&s.approvals).drop_ask_run(&project);
    lock(&s.running).retain(|p, _| !store::same_path(p, &project));
    emit_snapshot(&app);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    fn touch(p: &Path) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, b"").unwrap();
    }

    /// The regression this gap is about: Claude Code's own account cache can be stale, so Recheck
    /// must never paper over that by reusing a verdict Perch already had. Two calls with a counting
    /// fake must mean two real `claude auth status` calls, not one memoized and replayed.
    #[test]
    fn recheck_never_reuses_a_cached_auth_verdict() {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join(if cfg!(windows) { "claude.exe" } else { "claude" });
        touch(&bin);
        let calls = AtomicUsize::new(0);
        let version_of = |_: &Path| Some("2.1.282 (Claude Code)".to_string());
        let auth_status_of = |_: &Path| {
            calls.fetch_add(1, Ordering::SeqCst);
            AuthVerdict::Allowed { subscription: "max".to_string() }
        };
        let home = dir.path();

        let first = compute_recheck(Some(&bin), None, home, version_of, auth_status_of);
        assert!(matches!(first.auth, Some(AuthVerdict::Allowed { .. })));
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        let second = compute_recheck(Some(&bin), None, home, version_of, auth_status_of);
        assert!(matches!(second.auth, Some(AuthVerdict::Allowed { .. })));
        assert_eq!(calls.load(Ordering::SeqCst), 2, "a second recheck must call auth status again, not reuse the first result");
    }

    #[test]
    fn recheck_reports_no_auth_when_claude_code_itself_is_not_found() {
        let home = tempfile::tempdir().unwrap();
        let calls = AtomicUsize::new(0);
        let outcome = compute_recheck(None, None, home.path(), |_: &Path| None, |_: &Path| {
            calls.fetch_add(1, Ordering::SeqCst);
            AuthVerdict::Allowed { subscription: "max".to_string() }
        });
        assert!(outcome.located.is_err());
        assert!(outcome.auth.is_none());
        assert_eq!(calls.load(Ordering::SeqCst), 0, "auth status is only worth checking once a binary was found");
    }
}
