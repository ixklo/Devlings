use std::{sync::atomic::Ordering, time::Duration};

use tauri::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewWindow, Window, WindowEvent,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::{
    events::Mood,
    state::{self, AppState, Snapshot},
};

const TRAY_ID: &str = "perch-tray";

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

fn mood_words(m: Mood) -> &'static str {
    match m {
        Mood::Setup => "needs setup",
        Mood::NeedsYou => "needs you",
        Mood::Failed => "hit a problem",
        Mood::Working => "is working",
        Mood::Done => "is done",
        Mood::Listening => "is listening",
        Mood::Sleeping => "is sleeping",
        Mood::Idle => "is idle",
    }
}

pub fn update_tray_tooltip(app: &AppHandle, snap: &Snapshot) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_tooltip(Some(format!("{} {}", snap.config.pet_name, mood_words(snap.mood))));
    }
}

pub fn register_shortcut(app: &AppHandle) {
    let result = app.global_shortcut().on_shortcut("CommandOrControl+Alt+P", |app, _shortcut, event| {
        if event.state == ShortcutState::Pressed {
            let _ = toggle_panel(app);
        }
    });
    if let Err(e) = result {
        eprintln!("Perch: couldn't register Ctrl+Alt+P: {e}");
    }
}

pub fn place_pet(app: &AppHandle) {
    let pet = window(app, "pet");
    let saved = app.state::<AppState>().config.lock().unwrap().pet_position;
    let monitors = pet.available_monitors().unwrap_or_default();
    let on_screen = |x: i32, y: i32| {
        monitors.iter().any(|m| {
            let (p, s) = (m.position(), m.size());
            x >= p.x && y >= p.y && x < p.x + s.width as i32 - 40 && y < p.y + s.height as i32 - 40
        })
    };
    let pos = match saved {
        Some((x, y)) if on_screen(x, y) => PhysicalPosition::new(x, y),
        _ => {
            let Ok(Some(m)) = pet.primary_monitor() else { return };
            let size = pet.outer_size().unwrap_or(PhysicalSize::new(220, 210));
            let area = m.work_area();
            PhysicalPosition::new(
                area.position.x + area.size.width as i32 - size.width as i32 - 24,
                area.position.y + area.size.height as i32 - size.height as i32,
            )
        }
    };
    let _ = pet.set_position(pos);
}

fn position_panel(app: &AppHandle, panel: &WebviewWindow) -> tauri::Result<()> {
    let pet = window(app, "pet");
    let pos = pet.outer_position()?;
    let pet_size = pet.outer_size()?;
    let size = panel.outer_size()?;
    let monitor = match pet.current_monitor()? {
        Some(m) => Some(m),
        None => pet.primary_monitor()?,
    };
    let (mx, my, mw, mh) = monitor
        .map(|m| {
            let a = m.work_area();
            (a.position.x, a.position.y, a.size.width as i32, a.size.height as i32)
        })
        .unwrap_or((0, 0, 1920, 1080));
    let (w, h) = (size.width as i32, size.height as i32);
    let mut x = pos.x - w - 8;
    if x < mx {
        x = pos.x + pet_size.width as i32 + 8;
    }
    let x = x.clamp(mx, (mx + mw - w).max(mx));
    let y = (pos.y + pet_size.height as i32 - h).clamp(my, (my + mh - h).max(my));
    panel.set_position(PhysicalPosition::new(x, y))
}

pub fn open_panel(app: &AppHandle, view: &str) -> tauri::Result<()> {
    let panel = window(app, "panel");
    if !panel.is_visible()? {
        position_panel(app, &panel)?;
        panel.show()?;
    }
    if panel.is_minimized()? {
        panel.unminimize()?;
    }
    panel.set_focus()?;
    app.state::<AppState>().panel_open.store(true, Ordering::SeqCst);
    let _ = app.emit_to("panel", "panel-view", view);
    state::emit_snapshot(app);
    Ok(())
}

pub fn close_panel(app: &AppHandle) -> tauri::Result<()> {
    window(app, "panel").hide()?;
    app.state::<AppState>().panel_open.store(false, Ordering::SeqCst);
    state::emit_snapshot(app);
    Ok(())
}

pub fn toggle_panel(app: &AppHandle) -> tauri::Result<()> {
    let panel = window(app, "panel");
    if panel.is_visible()? && !panel.is_minimized()? {
        close_panel(app)
    } else {
        open_panel(app, "main")
    }
}

pub fn show_pet(app: &AppHandle) {
    let _ = window(app, "pet").show();
}

pub fn show_pet_menu(win: &Window) -> tauri::Result<()> {
    let app = win.app_handle();
    let menu = Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, "open-panel", "Open", true, None::<&str>)?,
            &MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?,
            &MenuItem::with_id(app, "hide-hour", "Hide for 1 hour", true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?,
        ],
    )?;
    win.popup_menu(&menu)
}

fn hide_pet_for_hour(app: &AppHandle) {
    let _ = window(app, "pet").hide();
    let handle = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(3600));
        show_pet(&handle);
    });
}

pub fn on_menu_event(app: &AppHandle, event: MenuEvent) {
    match event.id().as_ref() {
        "toggle-pet" => {
            let pet = window(app, "pet");
            if pet.is_visible().unwrap_or(true) {
                let _ = pet.hide();
            } else {
                let _ = pet.show();
            }
        }
        "open-panel" => {
            let _ = open_panel(app, "main");
        }
        "settings" => {
            let _ = open_panel(app, "settings");
        }
        "hide-hour" => hide_pet_for_hour(app),
        "quit" => app.exit(0),
        _ => {}
    }
}

pub fn on_window_event(win: &Window, event: &WindowEvent) {
    if let WindowEvent::CloseRequested { api, .. } = event {
        api.prevent_close();
        if win.label() == "panel" {
            let _ = close_panel(win.app_handle());
        } else {
            let _ = win.hide();
        }
    }
}
