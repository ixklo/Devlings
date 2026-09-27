use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use tauri::{AppHandle, Manager, Window};
use tauri_plugin_autostart::ManagerExt;

use crate::{
    approvals::{self, Decision},
    diagnostics,
    hooks_installer::{self, HookTarget, RelayExe},
    locks::lock,
    money_guard::AuthVerdict,
    overlay::{self, HitRect},
    pets::{self, PetInfo},
    runner::{self, AskRequest},
    shell::{self, SettingsView},
    state::{self, now_ms, AppState, Snapshot},
    store::{self, PermissionMode},
    transcript::{self, ChatTurn},
    trust,
};

type CmdResult<T> = Result<T, String>;

fn publish(app: &AppHandle) -> Snapshot {
    state::emit_snapshot(app);
    state::snapshot(app)
}

#[tauri::command]
pub fn get_snapshot(app: AppHandle) -> Snapshot {
    state::snapshot(&app)
}

#[tauri::command]
pub async fn recheck_setup(app: AppHandle) -> Snapshot {
    state::recheck_setup(&app);
    publish(&app)
}

#[tauri::command]
pub fn set_pet_name(app: AppHandle, name: String) -> CmdResult<Snapshot> {
    let name = store::validate_pet_name(&name)?;
    let s = app.state::<AppState>();
    lock(&s.config).pet_name = name;
    s.save_config();
    Ok(publish(&app))
}

fn install_on_port(app: &AppHandle, new_port: bool) -> CmdResult<Snapshot> {
    let s = app.state::<AppState>();
    let relay = hooks_installer::relay_exe()?;
    match &relay {
        RelayExe::Usable(_) => {}
        RelayExe::Ephemeral(exe) => {
            log::warn!("Devlings runs from a temporary location, so Stop, StopFailure and SessionEnd use HTTP ({})", exe.display())
        }
        RelayExe::Unsafe(exe) => {
            log::warn!("Devlings' path has shell characters, so Stop, StopFailure and SessionEnd use HTTP ({})", exe.display())
        }
    }
    let (port, token) = {
        let mut c = lock(&s.config);
        let port = match c.hook_port {
            Some(p) if !new_port => p,
            _ => hooks_installer::free_port().map_err(|e| e.to_string())?,
        };
        let token = c.hook_token.clone().unwrap_or_else(hooks_installer::new_token);
        c.hook_port = Some(port);
        c.hook_token = Some(token.clone());
        c.hooks_declined = false;
        (port, token)
    };
    s.save_config();
    let path = hooks_installer::settings_path();
    let target = HookTarget { port, token: &token, exe: relay.usable() };
    if let Err(e) = hooks_installer::install_file(&path, target, now_ms() / 1000) {
        log::error!("Installing hooks failed: {e}");
        return Err(e);
    }
    log::info!("Hooks installed in {} for port {port}", path.display());
    state::start_hook_server(app);
    state::refresh_hooks_installed(app);
    Ok(publish(app))
}

#[tauri::command]
pub async fn install_hooks(app: AppHandle) -> CmdResult<Snapshot> {
    install_on_port(&app, false)
}

#[tauri::command]
pub async fn move_hooks_port(app: AppHandle) -> CmdResult<Snapshot> {
    install_on_port(&app, true)
}

#[tauri::command]
pub fn uninstall_hooks(app: AppHandle) -> CmdResult<Snapshot> {
    match hooks_installer::uninstall_file(&hooks_installer::settings_path(), now_ms() / 1000) {
        Ok(changed) => log::info!("Hooks removed (settings.json changed: {changed})"),
        Err(e) => {
            log::error!("Removing hooks failed: {e}");
            return Err(e);
        }
    }
    let s = app.state::<AppState>();
    if let Some(server) = lock(&s.hook_server).take() {
        server.stop();
    }
    lock(&s.config).hooks_declined = true;
    s.save_config();
    state::refresh_hooks_installed(&app);
    Ok(publish(&app))
}

#[tauri::command]
pub fn decline_hooks(app: AppHandle) -> Snapshot {
    let s = app.state::<AppState>();
    lock(&s.config).hooks_declined = true;
    s.save_config();
    publish(&app)
}

#[tauri::command]
pub async fn set_claude_path(app: AppHandle, path: Option<String>) -> Snapshot {
    {
        let s = app.state::<AppState>();
        lock(&s.config).claude_path = path.filter(|p| !p.trim().is_empty());
        s.save_config();
    }
    state::recheck_setup(&app);
    publish(&app)
}

#[tauri::command]
pub fn add_project(app: AppHandle, path: String) -> CmdResult<Snapshot> {
    if !Path::new(&path).is_dir() {
        return Err("That folder doesn't exist.".into());
    }
    let s = app.state::<AppState>();
    lock(&s.projects).touch(&path, now_ms());
    s.save_projects();
    Ok(publish(&app))
}

#[tauri::command]
pub fn set_permission_mode(app: AppHandle, project: String, mode: PermissionMode) -> CmdResult<Snapshot> {
    let s = app.state::<AppState>();
    lock(&s.projects)
        .get_mut(&project)
        .ok_or("Unknown project.")?
        .permission_mode = mode;
    s.save_projects();
    Ok(publish(&app))
}

#[tauri::command]
pub fn new_conversation(app: AppHandle, project: String) -> CmdResult<Snapshot> {
    let s = app.state::<AppState>();
    lock(&s.approvals).deny_ask_run(&project, approvals::NEW_CHAT_MESSAGE);
    {
        let mut projects = lock(&s.projects);
        let p = projects.get_mut(&project).ok_or("Unknown project.")?;
        p.ask_session_id = None;
        p.transcript_path = None;
    }
    s.save_projects();
    Ok(publish(&app))
}

#[tauri::command]
pub fn load_conversation(app: AppHandle, project: String) -> Vec<ChatTurn> {
    let s = app.state::<AppState>();
    let path = lock(&s.projects).get(&project).and_then(|p| p.transcript_path.clone());
    path.map(|p| transcript::load(Path::new(&p))).unwrap_or_default()
}

#[tauri::command]
pub async fn ask(app: AppHandle, project: String, prompt: String) -> CmdResult<()> {
    let s = app.state::<AppState>();
    if prompt.trim().is_empty() {
        return Err("Type something first.".into());
    }
    if !lock(&s.config).credits_notice_seen {
        return Err("credits_notice".into());
    }
    if lock(&s.running).keys().any(|p| store::same_path(p, &project)) {
        return Err("Already working on this project.".into());
    }
    if !Path::new(&project).is_dir() {
        lock(&s.projects).remove(&project);
        s.save_projects();
        state::emit_snapshot(&app);
        return Err("That folder no longer exists, so it was removed from your projects.".into());
    }
    let bin = state::ensure_claude_located(&app)?;
    let verdict = runner::auth_status(&bin);
    *lock(&s.auth) = Some(verdict.clone());
    if let AuthVerdict::Refused { reason } = verdict {
        state::emit_snapshot(&app);
        return Err(reason);
    }
    let (mode, resume, perch_trusted) = {
        let mut projects = lock(&s.projects);
        projects.touch(&project, now_ms());
        let p = projects.get(&project);
        (
            p.map(|p| p.permission_mode).unwrap_or_default(),
            p.and_then(|p| p.ask_session_id.clone()),
            p.is_some_and(|p| p.trusted),
        )
    };
    s.save_projects();
    log::info!("Ask in {project} (mode {}, {})", mode.flag(), if resume.is_some() { "follow-up" } else { "new chat" });
    // Checked fresh at every Ask: trust and the folder's files can change between runs (design v1.0 D6).
    let policy = trust::ask_policy_here(Path::new(&project), perch_trusted);
    if let Some(notice) = &policy.notice {
        log::info!("Ask in {project}: untrusted folder, so it runs with --setting-sources user (found {})", notice.log);
    }
    let notice_session = resume.clone().unwrap_or_default();
    let mut req = AskRequest {
        bin,
        project: project.clone(),
        prompt,
        mode_flag: mode.flag(),
        resume,
        extra_args: policy.extra_args(),
    };
    // Check and reserve in one short lock (an update install claims itself before reading `running`),
    // then start the process outside it.
    {
        let mut running = lock(&s.running);
        crate::updater::ensure_not_installing(&app)?;
        reserve_run(&mut running, &project)?;
        // No run exists for this project, so a stop request left from the last one is stale.
        lock(&s.stop_requested).retain(|p| !store::same_path(p, &project));
    }
    let (child, stdin) = match runner::spawn(&req) {
        Ok(started) => started,
        // The remembered binary passed its last staleness check but vanished right before spawn
        // (e.g. the VS Code extension updated mid-session): re-locate once and retry.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            log::warn!("Ask in {project}: {} wasn't found; re-locating Claude Code", req.bin.display());
            state::recheck_setup(&app);
            let relocated = lock(&s.claude).as_ref().map(|l| l.path.clone()).map_err(|e| e.clone());
            let auth = lock(&s.auth).clone();
            // The re-located binary gets the same login check as the first one, before it runs.
            match retry_bin(relocated, auth) {
                Ok(bin) => {
                    req.bin = bin;
                    match runner::spawn(&req) {
                        Ok(started) => started,
                        Err(e2) => {
                            lock(&s.running).remove(&project);
                            log::error!("Ask in {project} couldn't start Claude Code after re-locating: {e2}");
                            return Err(format!("Couldn't start Claude Code: {e2}"));
                        }
                    }
                }
                Err(err) => {
                    lock(&s.running).remove(&project);
                    state::emit_snapshot(&app);
                    log::error!("Ask in {project}: not retried after re-locating Claude Code: {err}");
                    return Err(err);
                }
            }
        }
        Err(e) => {
            lock(&s.running).remove(&project);
            log::error!("Ask in {project} couldn't start Claude Code: {e}");
            return Err(format!("Couldn't start Claude Code: {e}"));
        }
    };
    let stopped_while_starting = {
        let mut running = lock(&s.running);
        running.insert(project.clone(), child.id());
        lock(&s.stop_requested).iter().any(|p| store::same_path(p, &project))
    };
    if stopped_while_starting {
        log::info!("Ask in {project} was stopped while it started");
        runner::kill_tree(child.id());
    }
    // Sent before the output reader starts, so the notice comes before anything the run says.
    if let Some(notice) = &policy.notice {
        let pet = lock(&s.config).pet_name.clone();
        state::emit_ask_notice(&app, &notice.event(&pet, &notice_session, &project, now_ms()));
    }
    state::emit_snapshot(&app);
    let handle = app.clone();
    std::thread::spawn(move || state::run_ask(handle, project, child, stdin));
    Ok(())
}

#[tauri::command]
pub fn stop_ask(app: AppHandle, project: String) {
    let s = app.state::<AppState>();
    let pid = {
        let running = lock(&s.running);
        let pid = running.iter().find(|(p, _)| store::same_path(p, &project)).map(|(_, pid)| *pid);
        if pid.is_some() {
            log::info!("Stop requested for the Ask in {project}");
            lock(&s.stop_requested).insert(project.clone());
        }
        pid
    };
    // Stop answers the run's pending permission requests first, then stops it as before.
    if lock(&s.approvals).deny_ask_run(&project, approvals::STOPPED_MESSAGE) > 0 {
        state::emit_snapshot(&app);
    }
    // A run that is still starting has no process yet (and `kill` of pid 0 would hit Devlings' own
    // process group); `ask` stops it as soon as it has one.
    if let Some(pid) = pid.filter(|&pid| pid != STARTING) {
        runner::kill_tree(pid);
    }
}

/// Trusts a project's folder in Devlings: its next Ask uses the folder's own Claude Code settings (design v1.0 D6).
#[tauri::command]
pub fn trust_project(app: AppHandle, project: String) -> CmdResult<Snapshot> {
    set_project_trust(&app, &project, true)
}

/// Undoes `trust_project`: the next Ask in that folder skips its project settings again, unless Claude Code trusts it.
#[tauri::command]
pub fn untrust_project(app: AppHandle, project: String) -> CmdResult<Snapshot> {
    set_project_trust(&app, &project, false)
}

fn set_project_trust(app: &AppHandle, project: &str, trusted: bool) -> CmdResult<Snapshot> {
    let s = app.state::<AppState>();
    {
        let mut projects = lock(&s.projects);
        if projects.set_trusted(project, trusted)? {
            // Saved atomically (temp file, then rename). A change that can't be saved is undone, so the UI never
            // shows trust that would be lost on restart.
            if let Err(e) = store::save(&s.data_dir.join("projects.json"), &*projects) {
                let _ = projects.set_trusted(project, !trusted);
                log::error!("Couldn't save projects.json: {e}");
                return Err(format!("Couldn't save that: {e}"));
            }
            log::info!("{} {project} in Devlings", if trusted { "Trusted" } else { "Stopped trusting" });
        }
    }
    Ok(publish(app))
}

/// The binary a retried Ask may run after re-locating Claude Code: only one whose fresh login check allows Asks.
fn retry_bin(located: Result<PathBuf, String>, auth: Option<AuthVerdict>) -> CmdResult<PathBuf> {
    let bin = located?;
    match auth {
        Some(AuthVerdict::Allowed { .. }) => Ok(bin),
        Some(AuthVerdict::Refused { reason }) => Err(reason),
        None => Err("Couldn't check Claude Code's login.".into()),
    }
}

/// The PID recorded for an Ask whose process is still starting.
const STARTING: u32 = 0;

/// Claims `project` for a new Ask run. The caller holds the `running` lock.
fn reserve_run(running: &mut HashMap<String, u32>, project: &str) -> CmdResult<()> {
    if running.keys().any(|p| store::same_path(p, project)) {
        return Err("Already working on this project.".into());
    }
    running.insert(project.to_string(), STARTING);
    Ok(())
}

#[tauri::command]
pub fn mark_credits_notice_seen(app: AppHandle) -> Snapshot {
    let s = app.state::<AppState>();
    lock(&s.config).credits_notice_seen = true;
    s.save_config();
    publish(&app)
}

#[tauri::command]
pub fn finish_onboarding(app: AppHandle) -> Snapshot {
    let s = app.state::<AppState>();
    // A fresh install learns about approvals during onboarding, so the upgrade intro card never shows.
    lock(&s.config).finish_onboarding();
    s.save_config();
    publish(&app)
}

/// The user's answer to a permission request from a card or the mini chat. The first answer wins.
#[tauri::command]
pub fn answer_approval(app: AppHandle, id: String, decision: Decision) -> CmdResult<Snapshot> {
    let resolved = lock(&app.state::<AppState>().approvals).answer(&id, decision)?;
    log::info!("Permission request for {} answered in Devlings ({decision:?})", approvals::log_safe(&resolved.raw_tool));
    state::after_resolved(&app, vec![resolved]);
    Ok(state::snapshot(&app))
}

#[tauri::command]
pub fn set_watch_approvals(app: AppHandle, enabled: bool) -> Snapshot {
    let s = app.state::<AppState>();
    // Decided under the registry lock, so a request arriving now is either not held or released here.
    // Turning it off answers everything held "no decision"; Claude Code's own prompts carry on.
    lock(&s.approvals).set_watching(enabled);
    lock(&s.config).set_watch_approvals(enabled);
    s.save_config();
    publish(&app)
}

#[tauri::command]
pub fn set_approval_hold(app: AppHandle, secs: u64) -> CmdResult<Snapshot> {
    let secs = store::validate_approval_hold(secs)?;
    let s = app.state::<AppState>();
    lock(&s.config).approval_hold_secs = secs;
    s.save_config();
    Ok(publish(&app))
}

#[tauri::command]
pub fn mark_approvals_intro_seen(app: AppHandle) -> Snapshot {
    let s = app.state::<AppState>();
    lock(&s.config).approvals_intro_seen = true;
    s.save_config();
    publish(&app)
}

#[tauri::command]
pub fn set_notifications(app: AppHandle, enabled: bool) -> Snapshot {
    let s = app.state::<AppState>();
    lock(&s.config).notifications = enabled;
    s.save_config();
    if enabled {
        state::refresh_system_notifications(&app);
    }
    publish(&app)
}

#[tauri::command]
pub fn set_launch_at_login(app: AppHandle, enabled: bool) -> CmdResult<Snapshot> {
    let launcher = app.autolaunch();
    let result = if enabled { launcher.enable() } else { launcher.disable() };
    result.map_err(|e| e.to_string())?;
    let s = app.state::<AppState>();
    lock(&s.config).launch_at_login = enabled;
    s.save_config();
    Ok(publish(&app))
}

#[tauri::command]
pub fn mark_viewed(app: AppHandle, session_id: String) -> Snapshot {
    lock(&app.state::<AppState>().threads).mark_viewed(&session_id);
    publish(&app)
}

#[tauri::command]
pub fn set_threads_collapsed(app: AppHandle, collapsed: bool) -> Snapshot {
    let s = app.state::<AppState>();
    lock(&s.config).threads_collapsed = collapsed;
    s.save_config();
    publish(&app)
}

#[tauri::command]
pub fn set_pet_scale(app: AppHandle, scale: f64) -> Snapshot {
    let s = app.state::<AppState>();
    lock(&s.config).pet_scale = store::clamp_pet_scale(scale);
    s.save_config();
    publish(&app)
}

#[tauri::command]
pub async fn list_pets(app: AppHandle) -> Vec<PetInfo> {
    state::refresh_pets(&app).into_iter().map(|p| p.info).collect()
}

#[tauri::command]
pub async fn get_pet_sprite(app: AppHandle, id: String) -> CmdResult<String> {
    let cached = lock(&app.state::<AppState>().pets).iter().find(|p| p.info.id == id).cloned();
    let pet = match cached {
        Some(p) => p,
        None => pets::resolve(&state::refresh_pets(&app), &id).cloned().ok_or("No pets found.")?,
    };
    pets::sprite_data_url(&pet)
}

#[tauri::command]
pub async fn set_pet(app: AppHandle, id: String) -> CmdResult<Snapshot> {
    choose_pet(&app, &id)?;
    Ok(publish(&app))
}

/// Switches the pet. A name the user chose stays; a default name follows the new pet
/// (`store::name_after_pet_change`).
pub fn choose_pet(app: &AppHandle, id: &str) -> CmdResult<()> {
    let all = state::refresh_pets(app);
    let Some(pet) = all.iter().find(|p| p.info.id == id) else {
        return Err("That pet wasn't found.".into());
    };
    let s = app.state::<AppState>();
    {
        let mut c = lock(&s.config);
        let old_name = all.iter().find(|p| p.info.id == c.pet_id).map(|p| p.info.display_name.as_str());
        if let Some(name) = store::name_after_pet_change(&c.pet_name, old_name, &pet.info.display_name) {
            c.pet_name = name;
        }
        c.pet_id = id.to_string();
    }
    s.save_config();
    Ok(())
}

#[tauri::command]
pub fn set_focused_thread(app: AppHandle, session_id: Option<String>) {
    *lock(&app.state::<AppState>().focused_thread) = session_id;
}

#[tauri::command]
pub fn set_hit_regions(app: AppHandle, regions: Vec<HitRect>) {
    *lock(&app.state::<AppState>().hit_regions) = Some(regions);
}

#[tauri::command]
pub fn open_settings(app: AppHandle, view: SettingsView) -> CmdResult<()> {
    shell::open_settings(&app, view).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn close_settings(app: AppHandle) -> CmdResult<()> {
    shell::close_settings(&app).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn open_project(app: AppHandle, path: String) -> CmdResult<()> {
    let dir = Path::new(&path);
    if !dir.is_absolute() || !dir.is_dir() {
        return Err("That folder doesn't exist.".into());
    }
    shell::open_project(&app, dir)
}

#[tauri::command]
pub async fn open_pets_folder(app: AppHandle) -> CmdResult<()> {
    let dir = pets::user_pets_dir(&app.state::<AppState>().data_dir);
    std::fs::create_dir_all(&dir).map_err(|e| format!("Couldn't create {}: {e}", dir.display()))?;
    shell::open_folder(&app, &dir)
}

#[tauri::command]
pub fn reset_pet_position(app: AppHandle) {
    overlay::reset_pet_position(&app);
}

#[tauri::command]
pub fn move_pet_by(app: AppHandle, dx: f64, dy: f64) {
    overlay::move_pet_by(&app, dx, dy);
}

#[tauri::command]
pub fn drag_pet_by(app: AppHandle, dx: f64, dy: f64) {
    overlay::drag_pet_by(&app, dx, dy);
}

#[tauri::command]
pub fn show_pet_menu(window: Window) -> CmdResult<()> {
    shell::show_pet_menu(&window).map_err(|e| e.to_string())
}

/// Redacted diagnostics for bug reports: versions, how Claude Code was found, hook status, and the log tail.
#[tauri::command]
pub async fn get_diagnostics(app: AppHandle) -> String {
    diagnostics::collect(&app)
}

#[tauri::command]
pub async fn open_log_folder(app: AppHandle) -> CmdResult<()> {
    let dir = diagnostics::log_dir(&app);
    std::fs::create_dir_all(&dir).map_err(|e| format!("Couldn't create {}: {e}", dir.display()))?;
    shell::open_folder(&app, &dir)
}

/// The pet's current `.stage` layout, for a page that missed the startup `pet-placement` event.
#[tauri::command]
pub fn get_pet_placement(app: AppHandle) -> Option<crate::overlay::Placement> {
    *lock(&app.state::<AppState>().placement)
}

#[tauri::command]
/// Saves the pet's spot once a drag settles. `x`/`y` are the window's position as the page sees it; the saved
/// position is the sprite's anchor (design D9), so the backend works it out from the window itself.
pub fn save_pet_position(app: AppHandle, x: i32, y: i32) {
    let _ = (x, y);
    overlay::remember_current(&app);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_retried_ask_needs_a_fresh_allowed_login() {
        let bin = || Ok(PathBuf::from("/opt/new/claude"));
        let allowed = Some(AuthVerdict::Allowed { subscription: "pro".into() });
        assert_eq!(retry_bin(bin(), allowed.clone()), Ok(PathBuf::from("/opt/new/claude")));
        let refused = Some(AuthVerdict::Refused { reason: "Claude Code isn't logged in.".into() });
        assert_eq!(retry_bin(bin(), refused), Err("Claude Code isn't logged in.".to_string()));
        assert!(retry_bin(bin(), None).is_err());
        assert_eq!(retry_bin(Err("Claude Code wasn't found.".into()), allowed), Err("Claude Code wasn't found.".to_string()));
    }

    #[test]
    fn an_ask_reserves_its_project_until_the_process_starts() {
        let mut running = HashMap::from([("C:\\code\\api".to_string(), 42)]);
        assert_eq!(reserve_run(&mut running, "C:\\code\\app"), Ok(()));
        assert_eq!(running.get("C:\\code\\app"), Some(&STARTING));
        // A second submit for the same project is refused, reserved or running.
        assert_eq!(reserve_run(&mut running, "C:\\code\\app\\"), Err("Already working on this project.".to_string()));
        assert_eq!(reserve_run(&mut running, "C:\\code\\api"), Err("Already working on this project.".to_string()));
        assert_eq!(running.len(), 2);
    }
}
