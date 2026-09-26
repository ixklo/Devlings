use std::{cmp::Reverse, collections::HashMap};

use serde::Serialize;

use crate::{
    events::{Kind, PetEvent, Source},
    store,
};

/// Running threads drop out of the bubbles when nothing has happened for this long.
pub const RUNNING_STALE_MS: i64 = 30 * 60_000;
/// Threads are forgotten this long after their last update, unless they are unread.
pub const KEEP_MS: i64 = 60 * 60_000;
pub const EXCERPT_CHARS: usize = 200;
const WRITING_LABEL: &str = "Writing a reply…";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ThreadStatus {
    Running,
    NeedsInput,
    Ready,
    Blocked,
    Idle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PetState {
    Idle,
    Running,
    NeedsInput,
    Ready,
    Blocked,
    Setup,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadInfo {
    pub session_id: String,
    pub project: String,
    pub project_name: String,
    pub source: Source,
    pub status: ThreadStatus,
    pub label: Option<String>,
    pub excerpt: Option<String>,
    pub updated_at: i64,
    pub unread: bool,
}

/// What one event did to the thread table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Applied {
    /// Something a bubble shows changed, or a thread appeared or disappeared.
    pub changed: bool,
    /// The thread just entered a status that deserves a notification.
    pub alert: Option<ThreadStatus>,
}

struct Entry {
    info: ThreadInfo,
    last_kind: Kind,
}

#[derive(Default)]
pub struct Threads {
    map: HashMap<String, Entry>,
}

fn status_for(kind: Kind) -> Option<ThreadStatus> {
    match kind {
        Kind::Prompt | Kind::Step | Kind::Blocked | Kind::ReplyDelta => Some(ThreadStatus::Running),
        Kind::NeedsYou => Some(ThreadStatus::NeedsInput),
        Kind::Done => Some(ThreadStatus::Ready),
        Kind::Failed => Some(ThreadStatus::Blocked),
        Kind::Started => Some(ThreadStatus::Idle),
        Kind::Ended => None,
    }
}

/// Bubble order, most urgent first.
fn rank(s: ThreadStatus) -> u8 {
    match s {
        ThreadStatus::NeedsInput => 0,
        ThreadStatus::Blocked => 1,
        ThreadStatus::Ready => 2,
        ThreadStatus::Running => 3,
        ThreadStatus::Idle => 4,
    }
}

fn is_result(s: ThreadStatus) -> bool {
    matches!(s, ThreadStatus::Ready | ThreadStatus::Blocked)
}

fn display_name(project: &str) -> String {
    let name = store::project_name(project);
    if name.is_empty() { "Claude Code".to_string() } else { name }
}

/// One line of at most EXCERPT_CHARS characters, or None when there is no text.
pub fn excerpt(text: &str) -> Option<String> {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.is_empty() {
        return None;
    }
    if flat.chars().count() <= EXCERPT_CHARS {
        return Some(flat);
    }
    let cut: String = flat.chars().take(EXCERPT_CHARS - 1).collect();
    Some(format!("{}…", cut.trim_end()))
}

pub fn is_visible(t: &ThreadInfo, now: i64) -> bool {
    match t.status {
        ThreadStatus::Idle => false,
        ThreadStatus::Running => now - t.updated_at < RUNNING_STALE_MS,
        _ => true,
    }
}

/// Overall pet state: setup > needs_input > blocked > ready > running > idle.
pub fn pet_state(visible: &[ThreadInfo], setup: bool) -> PetState {
    if setup {
        return PetState::Setup;
    }
    match visible.iter().map(|t| t.status).min_by_key(|s| rank(*s)) {
        Some(ThreadStatus::NeedsInput) => PetState::NeedsInput,
        Some(ThreadStatus::Blocked) => PetState::Blocked,
        Some(ThreadStatus::Ready) => PetState::Ready,
        Some(ThreadStatus::Running) => PetState::Running,
        Some(ThreadStatus::Idle) | None => PetState::Idle,
    }
}

impl Threads {
    pub fn apply(&mut self, e: &PetEvent) -> Applied {
        let Some(status) = status_for(e.kind) else {
            return Applied { changed: self.map.remove(&e.session_id).is_some(), alert: None };
        };
        let before = self.map.get(&e.session_id).map(|en| en.info.clone());
        let entry = self.map.entry(e.session_id.clone()).or_insert_with(|| Entry {
            info: ThreadInfo {
                session_id: e.session_id.clone(),
                project: e.project.clone(),
                project_name: display_name(&e.project),
                source: e.source,
                status,
                label: None,
                excerpt: None,
                updated_at: e.at,
                unread: false,
            },
            last_kind: e.kind,
        });
        let t = &mut entry.info;
        if !e.project.is_empty() && e.project != t.project {
            t.project = e.project.clone();
            t.project_name = display_name(&e.project);
        }
        t.source = e.source;
        t.updated_at = t.updated_at.max(e.at);
        if e.kind == Kind::ReplyDelta {
            if before.is_none() || entry.last_kind != Kind::ReplyDelta {
                t.label = Some(WRITING_LABEL.to_string());
            }
        } else if e.label.is_some() {
            t.label = e.label.clone();
        }
        entry.last_kind = e.kind;
        t.status = status;
        t.unread = is_result(status);
        t.excerpt = if is_result(status) { e.text.as_deref().and_then(excerpt) } else { None };

        let t = &entry.info;
        let entered = before.as_ref().is_none_or(|b| b.status != t.status);
        let alert = (entered && matches!(t.status, ThreadStatus::NeedsInput | ThreadStatus::Ready | ThreadStatus::Blocked))
            .then_some(t.status);
        let changed = before.as_ref().is_none_or(|b| {
            (b.status, &b.label, &b.excerpt, b.unread, &b.project) != (t.status, &t.label, &t.excerpt, t.unread, &t.project)
        });
        Applied { changed, alert }
    }

    /// Clears unread; a viewed ready or blocked thread goes idle. Returns whether anything changed.
    pub fn mark_viewed(&mut self, session_id: &str) -> bool {
        let Some(entry) = self.map.get_mut(session_id) else { return false };
        let t = &mut entry.info;
        let was = (t.status, t.unread);
        t.unread = false;
        if is_result(t.status) {
            t.status = ThreadStatus::Idle;
            t.excerpt = None;
        }
        was != (t.status, t.unread)
    }

    pub fn get(&self, session_id: &str) -> Option<&ThreadInfo> {
        self.map.get(session_id).map(|e| &e.info)
    }

    /// Threads shown as bubbles, most urgent first, then newest.
    pub fn visible(&self, now: i64) -> Vec<ThreadInfo> {
        let mut v: Vec<ThreadInfo> = self.map.values().map(|e| &e.info).filter(|t| is_visible(t, now)).cloned().collect();
        v.sort_by(|a, b| {
            (rank(a.status), Reverse(a.updated_at), &a.session_id).cmp(&(rank(b.status), Reverse(b.updated_at), &b.session_id))
        });
        v
    }

    /// Drops threads not updated for KEEP_MS unless they are unread. Returns whether any were dropped.
    pub fn prune(&mut self, now: i64) -> bool {
        let before = self.map.len();
        self.map.retain(|_, e| e.info.unread || now - e.info.updated_at < KEEP_MS);
        self.map.len() != before
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: i64 = 60_000;

    fn ev(sid: &str, kind: Kind, at: i64) -> PetEvent {
        PetEvent {
            session_id: sid.into(),
            project: "C:\\work\\proj".into(),
            source: Source::Watch,
            kind,
            label: Some(format!("{kind:?}")),
            text: None,
            at,
        }
    }

    fn with_text(mut e: PetEvent, text: &str) -> PetEvent {
        e.text = Some(text.into());
        e
    }

    fn status_after(kind: Kind) -> Option<ThreadStatus> {
        let mut t = Threads::default();
        t.apply(&ev("a", Kind::Step, 0));
        t.apply(&ev("a", kind, 1));
        t.get("a").map(|x| x.status)
    }

    fn ids(v: &[ThreadInfo]) -> Vec<&str> {
        v.iter().map(|t| t.session_id.as_str()).collect()
    }

    #[test]
    fn kinds_map_to_statuses() {
        assert_eq!(status_after(Kind::Prompt), Some(ThreadStatus::Running));
        assert_eq!(status_after(Kind::Step), Some(ThreadStatus::Running));
        assert_eq!(status_after(Kind::Blocked), Some(ThreadStatus::Running));
        assert_eq!(status_after(Kind::ReplyDelta), Some(ThreadStatus::Running));
        assert_eq!(status_after(Kind::NeedsYou), Some(ThreadStatus::NeedsInput));
        assert_eq!(status_after(Kind::Done), Some(ThreadStatus::Ready));
        assert_eq!(status_after(Kind::Failed), Some(ThreadStatus::Blocked));
        assert_eq!(status_after(Kind::Started), Some(ThreadStatus::Idle));
        assert_eq!(status_after(Kind::Ended), None);
    }

    #[test]
    fn results_are_unread_with_an_excerpt() {
        let mut t = Threads::default();
        t.apply(&ev("a", Kind::Step, 0));
        t.apply(&with_text(ev("a", Kind::Done, 1), "All\n\n  tests   pass."));
        let a = t.get("a").unwrap();
        assert!(a.unread);
        assert_eq!(a.excerpt.as_deref(), Some("All tests pass."));
        t.apply(&with_text(ev("b", Kind::Failed, 2), "Plan limit reached"));
        assert!(t.get("b").unwrap().unread);
        t.apply(&ev("a", Kind::Prompt, 3));
        let a = t.get("a").unwrap();
        assert!(!a.unread);
        assert_eq!(a.excerpt, None);
    }

    #[test]
    fn excerpts_are_one_line_and_capped() {
        assert_eq!(excerpt("  \n "), None);
        assert_eq!(excerpt("a\nb"), Some("a b".to_string()));
        let long = excerpt(&"x".repeat(500)).unwrap();
        assert_eq!(long.chars().count(), EXCERPT_CHARS);
        assert!(long.ends_with('…'));
        assert_eq!(excerpt(&"y".repeat(EXCERPT_CHARS)).unwrap().chars().count(), EXCERPT_CHARS);
    }

    #[test]
    fn viewing_a_result_makes_it_idle() {
        let mut t = Threads::default();
        t.apply(&ev("a", Kind::Done, 0));
        assert!(t.mark_viewed("a"));
        let a = t.get("a").unwrap();
        assert_eq!((a.status, a.unread), (ThreadStatus::Idle, false));
        assert!(t.visible(1).is_empty());
        assert!(!t.mark_viewed("a"));
        assert!(!t.mark_viewed("missing"));
        t.apply(&ev("b", Kind::Step, 0));
        assert!(!t.mark_viewed("b"));
        assert_eq!(t.get("b").unwrap().status, ThreadStatus::Running);
    }

    #[test]
    fn ended_removes_the_thread() {
        let mut t = Threads::default();
        t.apply(&ev("a", Kind::Step, 0));
        assert!(t.apply(&ev("a", Kind::Ended, 1)).changed);
        assert!(t.get("a").is_none());
        assert!(!t.apply(&ev("a", Kind::Ended, 2)).changed);
    }

    #[test]
    fn visibility_and_staleness() {
        let mut t = Threads::default();
        t.apply(&ev("idle", Kind::Started, 0));
        t.apply(&ev("run", Kind::Step, 0));
        t.apply(&ev("done", Kind::Done, 0));
        assert_eq!(ids(&t.visible(MIN)), vec!["done", "run"]);
        assert_eq!(ids(&t.visible(31 * MIN)), vec!["done"]);
    }

    #[test]
    fn sorted_by_priority_then_newest() {
        let mut t = Threads::default();
        t.apply(&ev("run-old", Kind::Step, 1));
        t.apply(&ev("run-new", Kind::Step, 9));
        t.apply(&ev("ready", Kind::Done, 8));
        t.apply(&ev("blocked", Kind::Failed, 2));
        t.apply(&ev("needs", Kind::NeedsYou, 3));
        assert_eq!(ids(&t.visible(10)), vec!["needs", "blocked", "ready", "run-new", "run-old"]);
    }

    #[test]
    fn pet_state_priority() {
        let mut t = Threads::default();
        assert_eq!(pet_state(&t.visible(0), false), PetState::Idle);
        t.apply(&ev("a", Kind::Step, 0));
        assert_eq!(pet_state(&t.visible(0), false), PetState::Running);
        t.apply(&ev("b", Kind::Done, 0));
        assert_eq!(pet_state(&t.visible(0), false), PetState::Ready);
        t.apply(&ev("c", Kind::Failed, 0));
        assert_eq!(pet_state(&t.visible(0), false), PetState::Blocked);
        t.apply(&ev("d", Kind::NeedsYou, 0));
        assert_eq!(pet_state(&t.visible(0), false), PetState::NeedsInput);
        assert_eq!(pet_state(&t.visible(0), true), PetState::Setup);
        assert_eq!(pet_state(&[], true), PetState::Setup);
    }

    #[test]
    fn stale_running_threads_leave_the_pet_idle() {
        let mut t = Threads::default();
        t.apply(&ev("a", Kind::Step, 0));
        assert_eq!(pet_state(&t.visible(31 * MIN), false), PetState::Idle);
    }

    #[test]
    fn prunes_after_an_hour_unless_unread() {
        let mut t = Threads::default();
        t.apply(&ev("old", Kind::Step, 0));
        t.apply(&ev("unread", Kind::Done, 0));
        t.apply(&ev("fresh", Kind::Step, 30 * MIN));
        assert!(t.prune(60 * MIN + 1));
        assert!(t.get("old").is_none());
        assert!(t.get("unread").is_some());
        assert!(t.get("fresh").is_some());
        assert!(!t.prune(60 * MIN + 2));
        t.mark_viewed("unread");
        assert!(t.prune(60 * MIN + 3));
        assert!(t.get("unread").is_none());
    }

    #[test]
    fn reply_delta_label() {
        let mut t = Threads::default();
        t.apply(&ev("a", Kind::Step, 0));
        let mut d = ev("a", Kind::ReplyDelta, 1);
        d.label = None;
        assert!(t.apply(&d).changed);
        assert_eq!(t.get("a").unwrap().label.as_deref(), Some(WRITING_LABEL));
        d.at = 2;
        assert!(!t.apply(&d).changed);
        t.apply(&ev("a", Kind::Step, 3));
        assert_eq!(t.get("a").unwrap().label.as_deref(), Some("Step"));
    }

    #[test]
    fn alerts_only_when_entering_a_notable_status() {
        let mut t = Threads::default();
        assert_eq!(t.apply(&ev("a", Kind::Step, 0)).alert, None);
        assert_eq!(t.apply(&ev("a", Kind::NeedsYou, 1)).alert, Some(ThreadStatus::NeedsInput));
        assert_eq!(t.apply(&ev("a", Kind::NeedsYou, 2)).alert, None);
        assert_eq!(t.apply(&ev("a", Kind::Done, 3)).alert, Some(ThreadStatus::Ready));
        assert_eq!(t.apply(&ev("b", Kind::Failed, 4)).alert, Some(ThreadStatus::Blocked));
        assert_eq!(t.apply(&ev("b", Kind::Failed, 5)).alert, None);
    }

    #[test]
    fn project_names() {
        let mut t = Threads::default();
        t.apply(&ev("a", Kind::Step, 0));
        assert_eq!(t.get("a").unwrap().project_name, "proj");
        let mut e = ev("b", Kind::Step, 0);
        e.project = String::new();
        t.apply(&e);
        assert_eq!(t.get("b").unwrap().project_name, "Claude Code");
    }

    #[test]
    fn serializes_to_the_frontend_contract() {
        let mut t = Threads::default();
        let mut e = ev("a", Kind::NeedsYou, 5);
        e.label = None;
        t.apply(&e);
        let v = serde_json::to_value(t.get("a").unwrap()).unwrap();
        assert_eq!(
            v,
            serde_json::json!({
                "sessionId": "a", "project": "C:\\work\\proj", "projectName": "proj", "source": "watch",
                "status": "needs_input", "label": null, "excerpt": null, "updatedAt": 5, "unread": false
            })
        );
        assert_eq!(serde_json::to_value(PetState::NeedsInput).unwrap(), "needs_input");
    }
}
