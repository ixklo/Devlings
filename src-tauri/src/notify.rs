//! Desktop notifications for threads (design v0.2 §3) and, on Windows, clicking one to open its thread.
//!
//! Windows shows toasts through `tauri-winrt-notification` directly, so the toast's `Activated` callback runs in
//! Perch's own process (it always is running, in the tray) and can open the thread. macOS and Linux keep
//! `tauri-plugin-notification`, where a click does nothing yet.

use serde::Serialize;
use serde_json::Value;
use tauri::AppHandle;

use crate::{
    events::{PetEvent, Source},
    store,
    threads::{self, ThreadInfo, ThreadStatus},
};

/// The pet window event a click sends. It's the tray's `pet-open` (v0.2 §5), so the pet opens the thread the same
/// way a click on its card does.
#[cfg_attr(not(any(windows, test)), allow(dead_code))]
pub const OPEN_EVENT: &str = "pet-open";

/// Body text is cut to this many characters.
const BODY_CHARS: usize = 120;

/// What a click on a notification opens: `pet-open {view: "thread", sessionId, project, source}`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenThread {
    pub view: &'static str,
    pub session_id: String,
    pub project: String,
    pub source: Source,
}

/// One notification: its text and the thread a click opens.
#[derive(Debug, Clone, PartialEq)]
pub struct Notice {
    pub title: String,
    pub body: String,
    /// Only Windows can act on a click (see the module docs).
    #[cfg_attr(not(any(windows, test)), allow(dead_code))]
    pub target: OpenThread,
}

/// The notification for a thread that just entered `status`, or None when that status doesn't get one. `thread` is
/// the thread as the table has it after the event, when it's there.
pub fn notice_for(pet: &str, ev: &PetEvent, status: ThreadStatus, thread: Option<&ThreadInfo>) -> Option<Notice> {
    let short = |t: &str| threads::excerpt(t).map(|x| x.chars().take(BODY_CHARS).collect::<String>());
    let body = match status {
        ThreadStatus::NeedsInput => "Needs your approval".to_string(),
        ThreadStatus::Ready => ev.text.as_deref().and_then(short).unwrap_or_else(|| "Done".to_string()),
        ThreadStatus::Blocked => ev
            .text
            .as_deref()
            .and_then(short)
            .or_else(|| ev.label.clone())
            .unwrap_or_else(|| "Something went wrong".to_string()),
        ThreadStatus::Running | ThreadStatus::Idle => return None,
    };
    let project_name = thread.map(|t| t.project_name.clone()).unwrap_or_else(|| store::project_name(&ev.project));
    Some(Notice {
        title: format!("{pet} · {project_name}"),
        body,
        target: OpenThread {
            view: "thread",
            session_id: ev.session_id.clone(),
            project: thread.map(|t| t.project.clone()).unwrap_or_else(|| ev.project.clone()),
            source: thread.map_or(ev.source, |t| t.source),
        },
    })
}

/// Where a click lands: the pet window. The real one is a thin wrapper over the app handle; tests record calls.
#[cfg_attr(not(any(windows, test)), allow(dead_code))]
pub trait PetWindow {
    fn show_and_focus(&self);
    fn emit(&self, event: &str, payload: Value);
}

/// A click on a notification: show the pet, then ask it to open the thread exactly as a click on its card would
/// (an Ask thread opens its mini chat; a Watch thread is marked seen and its project opens).
#[cfg_attr(not(any(windows, test)), allow(dead_code))]
pub fn activate(pet: &impl PetWindow, target: &OpenThread) {
    pet.show_and_focus();
    pet.emit(OPEN_EVENT, serde_json::to_value(target).unwrap_or(Value::Null));
}

/// The AppUserModelID for a Windows toast, chosen exactly as `tauri-plugin-notification` 2.4 chooses it: the bundle
/// identifier, which the installer's Start-menu shortcut registers, unless the executable sits in Cargo's
/// `target\debug` or `target\release` folder. None means "use PowerShell's id" (notify-rust's default), which
/// always shows.
#[cfg_attr(not(any(windows, test)), allow(dead_code))]
pub fn registered_app_id<'a>(exe_dir: &str, identifier: &'a str) -> Option<&'a str> {
    let dev = ["\\target\\debug", "\\target\\release"].iter().any(|suffix| exe_dir.ends_with(suffix));
    (!dev).then_some(identifier)
}

/// Shows a notification. Windows: a toast whose click opens the thread.
#[cfg(windows)]
pub fn show(app: &AppHandle, notice: Notice) {
    windows_toast::show(app, notice);
}

/// Shows a notification. macOS and Linux: through the plugin, as before; a click does nothing yet.
#[cfg(not(windows))]
pub fn show(app: &AppHandle, notice: Notice) {
    use tauri_plugin_notification::NotificationExt;
    let _ = app.notification().builder().title(notice.title).body(notice.body).show();
}

/// The WinRT side. Not unit-testable (it needs a desktop session), so it only builds the toast the plugin built
/// and hands clicks to `activate`.
#[cfg(windows)]
mod windows_toast {
    use serde_json::Value;
    use tauri::{AppHandle, Emitter, Manager};
    use tauri_winrt_notification::{Duration, Toast};

    use super::{activate, registered_app_id, Notice, PetWindow};
    use crate::{overlay, shell};

    struct Pet(AppHandle);

    impl PetWindow for Pet {
        fn show_and_focus(&self) {
            shell::show_pet(&self.0);
            if let Some(window) = self.0.get_webview_window(overlay::PET) {
                let _ = window.set_focus();
            }
        }

        fn emit(&self, event: &str, payload: Value) {
            let _ = self.0.emit_to(overlay::PET, event, payload);
        }
    }

    fn app_id(app: &AppHandle) -> String {
        let identifier = &app.config().identifier;
        let exe_dir = tauri::utils::platform::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|dir| dir.display().to_string()))
            .unwrap_or_default();
        registered_app_id(&exe_dir, identifier).unwrap_or(Toast::POWERSHELL_APP_ID).to_string()
    }

    pub fn show(app: &AppHandle, notice: Notice) {
        let app_id = app_id(app);
        let handle = app.clone();
        // Like the plugin: off the caller's thread, since showing a toast blocks briefly.
        tauri::async_runtime::spawn_blocking(move || {
            let Notice { title, body, target } = notice;
            let on_click = handle.clone();
            // The same toast the plugin (through notify-rust) showed: title, an empty first line, the body, silent,
            // short. There are no buttons, so any activation is a click on the toast itself.
            let result = Toast::new(&app_id)
                .title(&title)
                .text1("")
                .text2(&body)
                .sound(None)
                .duration(Duration::Short)
                .on_activated(move |_action| {
                    let (app, target) = (on_click.clone(), target.clone());
                    // WinRT calls this on its own thread; window work belongs on the main thread.
                    let _ = on_click.run_on_main_thread(move || activate(&Pet(app), &target));
                    Ok(())
                })
                .show();
            if let Err(e) = result {
                log::warn!("Couldn't show a notification: {e}");
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::Kind;
    use serde_json::json;
    use std::cell::RefCell;

    fn ev(kind: Kind, source: Source, label: Option<&str>, text: Option<&str>) -> PetEvent {
        PetEvent {
            session_id: "s1".into(),
            project: "C:\\Users\\me\\proj".into(),
            source,
            kind,
            label: label.map(Into::into),
            text: text.map(Into::into),
            at: 1,
        }
    }

    fn thread(project: &str, name: &str, source: Source) -> ThreadInfo {
        ThreadInfo {
            session_id: "s1".into(),
            project: project.into(),
            project_name: name.into(),
            source,
            status: ThreadStatus::Ready,
            label: None,
            excerpt: None,
            updated_at: 1,
            unread: true,
        }
    }

    fn body(status: ThreadStatus, label: Option<&str>, text: Option<&str>) -> Option<String> {
        notice_for("Mochi", &ev(Kind::Done, Source::Watch, label, text), status, None).map(|n| n.body)
    }

    #[test]
    fn bodies_say_what_happened() {
        assert_eq!(body(ThreadStatus::NeedsInput, Some("x"), Some("y")).as_deref(), Some("Needs your approval"));
        assert_eq!(body(ThreadStatus::Ready, None, Some("All\n\n tests pass.")).as_deref(), Some("All tests pass."));
        assert_eq!(body(ThreadStatus::Ready, Some("Done"), None).as_deref(), Some("Done"));
        assert_eq!(body(ThreadStatus::Ready, None, Some("  ")).as_deref(), Some("Done"));
        assert_eq!(body(ThreadStatus::Blocked, Some("Plan limit reached"), Some("Resets in 1h 0m.")).as_deref(), Some("Resets in 1h 0m."));
        assert_eq!(body(ThreadStatus::Blocked, Some("Plan limit reached"), None).as_deref(), Some("Plan limit reached"));
        assert_eq!(body(ThreadStatus::Blocked, None, None).as_deref(), Some("Something went wrong"));
        let long = body(ThreadStatus::Ready, None, Some(&"x".repeat(500))).unwrap();
        assert_eq!(long.chars().count(), BODY_CHARS);
    }

    #[test]
    fn working_and_idle_threads_get_no_notification() {
        assert_eq!(body(ThreadStatus::Running, None, Some("x")), None);
        assert_eq!(body(ThreadStatus::Idle, None, Some("x")), None);
    }

    #[test]
    fn title_and_target_come_from_the_thread() {
        let t = thread("C:\\Users\\me\\proj", "proj", Source::Ask);
        let n = notice_for("Mochi", &ev(Kind::Done, Source::Ask, None, Some("Hi")), ThreadStatus::Ready, Some(&t)).unwrap();
        assert_eq!(n.title, "Mochi · proj");
        assert_eq!(n.body, "Hi");
        assert_eq!(
            n.target,
            OpenThread { view: "thread", session_id: "s1".into(), project: "C:\\Users\\me\\proj".into(), source: Source::Ask }
        );
    }

    #[test]
    fn without_a_thread_the_event_fills_in() {
        let n = notice_for("Mochi", &ev(Kind::NeedsYou, Source::Watch, None, None), ThreadStatus::NeedsInput, None).unwrap();
        assert_eq!(n.title, "Mochi · proj");
        assert_eq!(n.target.project, "C:\\Users\\me\\proj");
        assert_eq!(n.target.source, Source::Watch);
    }

    #[derive(Default)]
    struct FakePet {
        calls: RefCell<Vec<String>>,
        emitted: RefCell<Vec<(String, Value)>>,
    }

    impl PetWindow for FakePet {
        fn show_and_focus(&self) {
            self.calls.borrow_mut().push("show".into());
        }

        fn emit(&self, event: &str, payload: Value) {
            self.calls.borrow_mut().push("emit".into());
            self.emitted.borrow_mut().push((event.to_string(), payload));
        }
    }

    fn target(sid: &str, project: &str, source: Source) -> OpenThread {
        OpenThread { view: "thread", session_id: sid.into(), project: project.into(), source }
    }

    #[test]
    fn a_click_shows_the_pet_then_asks_it_to_open_the_thread() {
        let pet = FakePet::default();
        activate(&pet, &target("s1", "C:\\Users\\me\\proj", Source::Ask));
        assert_eq!(*pet.calls.borrow(), vec!["show", "emit"], "the pet is visible before it opens the thread");
        assert_eq!(
            *pet.emitted.borrow(),
            vec![(
                "pet-open".to_string(),
                json!({"view": "thread", "sessionId": "s1", "project": "C:\\Users\\me\\proj", "source": "ask"})
            )]
        );
    }

    #[test]
    fn each_click_opens_its_own_thread() {
        let pet = FakePet::default();
        let first = target("a", "C:\\Users\\me\\one", Source::Ask);
        let second = target("b", "C:\\Users\\me\\two", Source::Watch);
        activate(&pet, &second);
        activate(&pet, &first);
        let opened: Vec<(Value, Value)> =
            pet.emitted.borrow().iter().map(|(_, p)| (p["sessionId"].clone(), p["source"].clone())).collect();
        assert_eq!(opened, vec![(json!("b"), json!("watch")), (json!("a"), json!("ask"))]);
    }

    #[test]
    fn toasts_use_the_installed_app_id_except_from_cargo_target_folders() {
        let id = "io.github.perchpet.perch";
        assert_eq!(registered_app_id("C:\\Users\\me\\AppData\\Local\\Perch", id), Some(id));
        assert_eq!(registered_app_id("C:\\Program Files\\Perch", id), Some(id));
        assert_eq!(registered_app_id("C:\\Users\\me\\perch\\src-tauri\\target\\debug", id), None);
        assert_eq!(registered_app_id("C:\\Users\\me\\perch\\src-tauri\\target\\release", id), None);
        assert_eq!(registered_app_id("C:\\Users\\me\\perch\\target\\debug\\deps", id), Some(id), "only the folder itself, as in the plugin");
        assert_eq!(registered_app_id("", id), Some(id));
    }
}
