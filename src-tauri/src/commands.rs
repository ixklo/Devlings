use std::path::Path;

use tauri::{AppHandle, Manager, Window};
use tauri_plugin_autostart::ManagerExt;

use crate::{
    hooks_installer,
    money_guard::AuthVerdict,
    runner::{self, AskRequest},
    shell,
    state::{self, now_ms, AppState, Snapshot},
    store::{self, PermissionMode},
    transcript::{self, ChatTurn},
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
    s.config.lock().unwrap().pet_name = name.clone();
    s.save_config();
    if let Some(panel) = app.get_webview_window("panel") {
        let _ = panel.set_title(&name);
    }
    Ok(publish(&app))
}

fn install_on_port(app: &AppHandle, new_port: bool) -> CmdResult<Snapshot> {
    let s = app.state::<AppState>();
    let (port, token) = {
        let mut c = s.config.lock().unwrap();
        if new_port || c.hook_port.is_none() {
            c.hook_port = Some(hooks_installer::free_port().map_err(|e| e.to_string())?);
        }
        if c.hook_token.is_none() {
            c.hook_token = Some(hooks_installer::new_token());
        }
        c.hooks_declined = false;
        (c.hook_port.unwrap(), c.hook_token.clone().unwrap())
    };
    s.save_config();
    hooks_installer::install_file(&hooks_installer::settings_path(), port, &token, now_ms() / 1000)?;
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
    hooks_installer::uninstall_file(&hooks_installer::settings_path(), now_ms() / 1000)?;
    let s = app.state::<AppState>();
    if let Some(server) = s.hook_server.lock().unwrap().take() {
        server.stop();
    }
    s.config.lock().unwrap().hooks_declined = true;
    s.save_config();
    state::refresh_hooks_installed(&app);
    Ok(publish(&app))
}

#[tauri::command]
pub fn decline_hooks(app: AppHandle) -> Snapshot {
    let s = app.state::<AppState>();
    s.config.lock().unwrap().hooks_declined = true;
    s.save_config();
    publish(&app)
}

#[tauri::command]
pub async fn set_claude_path(app: AppHandle, path: Option<String>) -> Snapshot {
    {
        let s = app.state::<AppState>();
        s.config.lock().unwrap().claude_path = path.filter(|p| !p.trim().is_empty());
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
    s.projects.lock().unwrap().touch(&path, now_ms());
    s.save_projects();
    Ok(publish(&app))
}

#[tauri::command]
pub fn set_permission_mode(app: AppHandle, project: String, mode: PermissionMode) -> CmdResult<Snapshot> {
    let s = app.state::<AppState>();
    s.projects
        .lock()
        .unwrap()
        .get_mut(&project)
        .ok_or("Unknown project.")?
        .permission_mode = mode;
    s.save_projects();
    Ok(publish(&app))
}

#[tauri::command]
pub fn new_conversation(app: AppHandle, project: String) -> CmdResult<Snapshot> {
    let s = app.state::<AppState>();
    {
        let mut projects = s.projects.lock().unwrap();
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
    let path = s.projects.lock().unwrap().get(&project).and_then(|p| p.transcript_path.clone());
    path.map(|p| transcript::load(Path::new(&p))).unwrap_or_default()
}

#[tauri::command]
pub async fn ask(app: AppHandle, project: String, prompt: String) -> CmdResult<()> {
    let s = app.state::<AppState>();
    if prompt.trim().is_empty() {
        return Err("Type something first.".into());
    }
    if !s.config.lock().unwrap().credits_notice_seen {
        return Err("credits_notice".into());
    }
    if s.running.lock().unwrap().keys().any(|p| store::same_path(p, &project)) {
        return Err("Already working on this project.".into());
    }
    if !Path::new(&project).is_dir() {
        s.projects.lock().unwrap().remove(&project);
        s.save_projects();
        state::emit_snapshot(&app);
        return Err("That folder no longer exists, so it was removed from your projects.".into());
    }
    let bin = s.claude.lock().unwrap().as_ref().map(|l| l.path.clone()).map_err(|e| e.clone())?;
    let verdict = runner::auth_status(&bin);
    *s.auth.lock().unwrap() = Some(verdict.clone());
    if let AuthVerdict::Refused { reason } = verdict {
        state::emit_snapshot(&app);
        return Err(reason);
    }
    let (mode, resume) = {
        let mut projects = s.projects.lock().unwrap();
        projects.touch(&project, now_ms());
        let p = projects.get(&project).expect("project was just touched");
        (p.permission_mode, p.ask_session_id.clone())
    };
    s.save_projects();
    let req = AskRequest { bin, project: project.clone(), prompt, mode_flag: mode.flag(), resume };
    let child = runner::spawn(&req).map_err(|e| format!("Couldn't start Claude Code: {e}"))?;
    s.running.lock().unwrap().insert(project.clone(), child.id());
    state::emit_snapshot(&app);
    let handle = app.clone();
    std::thread::spawn(move || state::run_ask(handle, project, child));
    Ok(())
}

#[tauri::command]
pub fn stop_ask(app: AppHandle, project: String) {
    let s = app.state::<AppState>();
    let pid = s
        .running
        .lock()
        .unwrap()
        .iter()
        .find(|(p, _)| store::same_path(p, &project))
        .map(|(_, pid)| *pid);
    if let Some(pid) = pid {
        s.stop_requested.lock().unwrap().insert(project);
        runner::kill_tree(pid);
    }
}

#[tauri::command]
pub fn mark_credits_notice_seen(app: AppHandle) -> Snapshot {
    let s = app.state::<AppState>();
    s.config.lock().unwrap().credits_notice_seen = true;
    s.save_config();
    publish(&app)
}

#[tauri::command]
pub fn finish_onboarding(app: AppHandle) -> Snapshot {
    let s = app.state::<AppState>();
    s.config.lock().unwrap().onboarded = true;
    s.save_config();
    publish(&app)
}

#[tauri::command]
pub fn set_notifications(app: AppHandle, enabled: bool) -> Snapshot {
    let s = app.state::<AppState>();
    s.config.lock().unwrap().notifications = enabled;
    s.save_config();
    publish(&app)
}

#[tauri::command]
pub fn set_launch_at_login(app: AppHandle, enabled: bool) -> CmdResult<Snapshot> {
    let launcher = app.autolaunch();
    let result = if enabled { launcher.enable() } else { launcher.disable() };
    result.map_err(|e| e.to_string())?;
    let s = app.state::<AppState>();
    s.config.lock().unwrap().launch_at_login = enabled;
    s.save_config();
    Ok(publish(&app))
}

#[tauri::command]
pub fn toggle_panel(app: AppHandle) -> CmdResult<()> {
    shell::toggle_panel(&app).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn close_panel(app: AppHandle) -> CmdResult<()> {
    shell::close_panel(&app).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn show_pet_menu(window: Window) -> CmdResult<()> {
    shell::show_pet_menu(&window).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_pet_position(app: AppHandle, x: i32, y: i32) {
    let s = app.state::<AppState>();
    s.config.lock().unwrap().pet_position = Some((x, y));
    s.save_config();
}
