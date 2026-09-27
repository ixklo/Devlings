use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    process::Stdio,
    sync::atomic::Ordering,
    time::Duration,
};

use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{
    menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, WebviewWindow, Window, WindowEvent,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use tauri_plugin_opener::OpenerExt;

use crate::{
    commands,
    locks::lock,
    overlay, pets, runner,
    state::{self, AppState, Snapshot},
    threads::PetState,
};

const TRAY_ID: &str = "perch-tray";
pub const SETTINGS: &str = "settings";
const PET_ITEM_PREFIX: &str = "pet:";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SettingsView {
    Settings,
    Onboarding,
}

fn window(app: &AppHandle, label: &str) -> WebviewWindow {
    app.get_webview_window(label)
        .unwrap_or_else(|| panic!("window '{label}' is declared in tauri.conf.json"))
}

pub fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let menu = Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, "toggle-pet", "Show/Hide pet", true, None::<&str>)?,
            &MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?,
        ],
    )?;
    let mut builder = TrayIconBuilder::with_id(TRAY_ID).tooltip("Perch").menu(&menu);
    if let Some(icon) = app.default_window_icon().cloned() {
        builder = builder.icon(icon);
    }
    builder.build(app)?;
    Ok(())
}

fn state_words(s: PetState) -> &'static str {
    match s {
        PetState::Setup => "needs setup",
        PetState::NeedsInput => "needs you",
        PetState::Blocked => "hit a problem",
        PetState::Ready => "has a reply",
        PetState::Running => "is working",
        PetState::Idle => "is idle",
    }
}

pub fn update_tray_tooltip(app: &AppHandle, snap: &Snapshot) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_tooltip(Some(format!("{} {}", snap.config.pet_name, state_words(snap.pet_state))));
    }
}

pub fn register_shortcut(app: &AppHandle) {
    let result = app.global_shortcut().on_shortcut("CommandOrControl+Alt+P", |app, _shortcut, event| {
        if event.state == ShortcutState::Pressed {
            toggle_pet(app, true);
        }
    });
    if let Err(e) = result {
        log::warn!("Couldn't register Ctrl+Alt+P: {e}");
    }
}

/// Every show or hide bumps the generation, so a pending "hide for 1 hour" timer knows it was overridden.
fn bump_visibility(app: &AppHandle) -> u64 {
    app.state::<AppState>().pet_visibility_gen.fetch_add(1, Ordering::SeqCst) + 1
}

pub fn show_pet(app: &AppHandle) {
    bump_visibility(app);
    let _ = window(app, overlay::PET).show();
}

fn hide_pet(app: &AppHandle) -> u64 {
    let generation = bump_visibility(app);
    let _ = window(app, overlay::PET).hide();
    generation
}

/// Shows or hides the pet. When `compose` is set, showing it also focuses it and opens the composer.
pub fn toggle_pet(app: &AppHandle, compose: bool) {
    let pet = window(app, overlay::PET);
    if pet.is_visible().unwrap_or(true) {
        hide_pet(app);
        return;
    }
    show_pet(app);
    if compose {
        let _ = pet.set_focus();
        let _ = app.emit_to(overlay::PET, "pet-open", json!({ "view": "compose" }));
    }
}

fn hide_pet_for_hour(app: &AppHandle) {
    let generation = hide_pet(app);
    let handle = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(3600));
        if handle.state::<AppState>().pet_visibility_gen.load(Ordering::SeqCst) == generation {
            show_pet(&handle);
        }
    });
}

pub fn open_settings(app: &AppHandle, view: SettingsView) -> tauri::Result<()> {
    let settings = window(app, SETTINGS);
    if !settings.is_visible()? {
        settings.show()?;
    }
    if settings.is_minimized()? {
        settings.unminimize()?;
    }
    settings.set_focus()?;
    app.emit_to(SETTINGS, "settings-view", view)
}

pub fn close_settings(app: &AppHandle) -> tauri::Result<()> {
    window(app, SETTINGS).hide()
}

pub fn show_pet_menu(win: &Window) -> tauri::Result<()> {
    let app = win.app_handle();
    let all = state::refresh_pets(app);
    let wanted = lock(&app.state::<AppState>().config).pet_id.clone();
    let current = pets::resolve(&all, &wanted).map(|p| p.info.id.clone());
    let change = Submenu::with_id(app, "change-pet", "Change pet", true)?;
    if all.is_empty() {
        change.append(&MenuItem::with_id(app, "no-pets", "No pets found", false, None::<&str>)?)?;
    }
    for p in &all {
        let checked = current.as_deref() == Some(p.info.id.as_str());
        let id = format!("{PET_ITEM_PREFIX}{}", p.info.id);
        change.append(&CheckMenuItem::with_id(app, id, &p.info.display_name, true, checked, None::<&str>)?)?;
    }
    let menu = Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?,
            &change,
            &MenuItem::with_id(app, "hide-hour", "Hide for 1 hour", true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?,
        ],
    )?;
    win.popup_menu(&menu)
}

pub fn on_menu_event(app: &AppHandle, event: MenuEvent) {
    let id = event.id().as_ref();
    if let Some(pet_id) = id.strip_prefix(PET_ITEM_PREFIX) {
        if commands::choose_pet(app, pet_id).is_ok() {
            state::emit_snapshot(app);
        }
        return;
    }
    match id {
        "toggle-pet" => toggle_pet(app, false),
        "settings" => {
            let _ = open_settings(app, SettingsView::Settings);
        }
        "hide-hour" => hide_pet_for_hour(app),
        "quit" => app.exit(0),
        _ => {}
    }
}

pub fn on_window_event(win: &Window, event: &WindowEvent) {
    if let WindowEvent::CloseRequested { api, .. } = event {
        api.prevent_close();
        if win.label() == overlay::PET {
            hide_pet(win.app_handle());
        } else {
            let _ = win.hide();
        }
    }
}

fn vscode_cli_name() -> &'static str {
    if cfg!(windows) { "code.cmd" } else { "code" }
}

/// VS Code's `code` launcher on PATH, if any.
pub fn find_on_path(name: &str, path_env: Option<&OsStr>) -> Option<PathBuf> {
    std::env::split_paths(path_env?).map(|dir| dir.join(name)).find(|p| p.is_file())
}

/// Opens a folder in VS Code when its CLI is on PATH, otherwise in the file manager.
pub fn open_project(app: &AppHandle, dir: &Path) -> Result<(), String> {
    if let Some(code) = find_on_path(vscode_cli_name(), std::env::var_os("PATH").as_deref()) {
        // Rust runs a .cmd through cmd.exe itself and escapes the arguments for it.
        let spawned = runner::background_command(&code)
            .arg(dir)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        if spawned.is_ok() {
            return Ok(());
        }
    }
    open_folder(app, dir)
}

pub fn open_folder(app: &AppHandle, dir: &Path) -> Result<(), String> {
    match app.opener().open_path(dir.to_string_lossy(), None::<&str>) {
        Ok(()) => Ok(()),
        Err(e) => {
            #[cfg(windows)]
            if runner::background_command(Path::new("explorer")).arg(dir).spawn().is_ok() {
                return Ok(());
            }
            Err(format!("Couldn't open {}: {e}", dir.display()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_first_match_on_path() {
        let t = tempfile::tempdir().unwrap();
        let (a, b) = (t.path().join("a"), t.path().join("b"));
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(b.join("code.cmd")).unwrap();
        std::fs::write(a.join("code.cmd"), "").unwrap();
        let path = std::env::join_paths([b.clone(), a.clone()]).unwrap();
        assert_eq!(find_on_path("code.cmd", Some(&path)), Some(a.join("code.cmd")));
        assert_eq!(find_on_path("missing", Some(&path)), None);
        assert_eq!(find_on_path("code.cmd", None), None);
    }

    #[test]
    fn settings_views_use_the_contract_strings() {
        assert_eq!(serde_json::to_value(SettingsView::Onboarding).unwrap(), "onboarding");
        assert_eq!(serde_json::from_str::<SettingsView>("\"settings\"").unwrap(), SettingsView::Settings);
        assert!(serde_json::from_str::<SettingsView>("\"main\"").is_err());
    }

    /// A folder name with shell metacharacters reaches a .cmd launcher as one literal argument.
    #[cfg(windows)]
    #[test]
    fn cmd_launchers_get_paths_verbatim() {
        let t = tempfile::tempdir().unwrap();
        let out = t.path().join("out.txt");
        let script = t.path().join("fake code.cmd");
        std::fs::write(&script, format!("@echo off\r\n>\"{}\" echo(%1\r\n", out.display())).unwrap();
        let dir = t.path().join("R&D (x) 100%");
        std::fs::create_dir_all(&dir).unwrap();
        let status = runner::background_command(&script).arg(&dir).status().unwrap();
        assert!(status.success());
        assert_eq!(std::fs::read_to_string(&out).unwrap().trim().trim_matches('"'), dir.display().to_string());
    }
}
