use std::collections::HashMap;

use serde::Serialize;

use crate::events::{Kind, Mood, PetEvent, Source};

pub const FAILED_WINDOW_MS: i64 = 30_000;
pub const DONE_WINDOW_MS: i64 = 8_000;
pub const SLEEP_AFTER_MS: i64 = 10 * 60_000;
pub const WORKING_STALE_MS: i64 = 30 * 60_000;
pub const KEEP_MS: i64 = 60 * 60_000;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub session_id: String,
    pub project: String,
    pub source: Source,
    pub state: Kind,
    pub label: Option<String>,
    pub last_at: i64,
}

pub struct Sessions {
    map: HashMap<String, SessionInfo>,
    created_at: i64,
    last_event_at: Option<i64>,
    last_failed_at: Option<i64>,
    last_done_at: Option<i64>,
}

fn is_working(k: Kind) -> bool {
    matches!(k, Kind::Prompt | Kind::Step | Kind::Blocked | Kind::ReplyDelta)
}

impl Sessions {
    pub fn new(now: i64) -> Self {
        Self { map: HashMap::new(), created_at: now, last_event_at: None, last_failed_at: None, last_done_at: None }
    }

    pub fn apply(&mut self, e: &PetEvent) {
        self.last_event_at = Some(e.at);
        match e.kind {
            Kind::Failed => self.last_failed_at = Some(e.at),
            Kind::Done => self.last_done_at = Some(e.at),
            _ => {}
        }
        let entry = self.map.entry(e.session_id.clone()).or_insert_with(|| SessionInfo {
            session_id: e.session_id.clone(),
            project: e.project.clone(),
            source: e.source,
            state: e.kind,
            label: None,
            last_at: e.at,
        });
        if !e.project.is_empty() {
            entry.project = e.project.clone();
        }
        entry.source = e.source;
        entry.last_at = e.at;
        if e.kind == Kind::ReplyDelta && entry.state != Kind::ReplyDelta {
            entry.label = Some("Writing a reply…".to_string());
        } else if e.label.is_some() {
            entry.label = e.label.clone();
        }
        entry.state = e.kind;
    }

    pub fn mood(&self, now: i64, panel_open: bool, setup: bool) -> Mood {
        let within = |t: Option<i64>, window: i64| t.map_or(false, |t| now - t < window);
        if setup {
            Mood::Setup
        } else if self.map.values().any(|s| s.state == Kind::NeedsYou) {
            Mood::NeedsYou
        } else if within(self.last_failed_at, FAILED_WINDOW_MS) {
            Mood::Failed
        } else if self.map.values().any(|s| is_working(s.state) && now - s.last_at < WORKING_STALE_MS) {
            Mood::Working
        } else if within(self.last_done_at, DONE_WINDOW_MS) {
            Mood::Done
        } else if panel_open {
            Mood::Listening
        } else if now - self.last_event_at.unwrap_or(self.created_at) >= SLEEP_AFTER_MS {
            Mood::Sleeping
        } else {
            Mood::Idle
        }
    }

    pub fn list(&self) -> Vec<SessionInfo> {
        let mut v: Vec<SessionInfo> = self.map.values().cloned().collect();
        v.sort_by(|a, b| b.last_at.cmp(&a.last_at));
        v
    }

    pub fn prune(&mut self, now: i64) {
        self.map.retain(|_, s| now - s.last_at < KEEP_MS);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: i64 = 60_000;

    fn ev(sid: &str, kind: Kind, at: i64) -> PetEvent {
        PetEvent {
            session_id: sid.into(),
            project: "C:\\proj".into(),
            source: Source::Watch,
            kind,
            label: Some(format!("{kind:?}")),
            text: None,
            at,
        }
    }

    #[test]
    fn idle_then_sleeping() {
        let s = Sessions::new(0);
        assert_eq!(s.mood(1_000, false, false), Mood::Idle);
        assert_eq!(s.mood(10 * MIN, false, false), Mood::Sleeping);
    }

    #[test]
    fn working_goes_stale() {
        let mut s = Sessions::new(0);
        s.apply(&ev("a", Kind::Step, 1_000));
        assert_eq!(s.mood(2_000, false, false), Mood::Working);
        assert_eq!(s.mood(1_000 + 31 * MIN, false, false), Mood::Sleeping);
    }

    #[test]
    fn started_is_not_working() {
        let mut s = Sessions::new(0);
        s.apply(&ev("a", Kind::Started, 1_000));
        assert_eq!(s.mood(2_000, false, false), Mood::Idle);
    }

    #[test]
    fn needs_you_wins_then_clears() {
        let mut s = Sessions::new(0);
        s.apply(&ev("a", Kind::Step, 1_000));
        s.apply(&ev("b", Kind::NeedsYou, 2_000));
        assert_eq!(s.mood(3_000, true, false), Mood::NeedsYou);
        s.apply(&ev("b", Kind::Step, 4_000));
        assert_eq!(s.mood(5_000, false, false), Mood::Working);
    }

    #[test]
    fn failed_and_done_windows() {
        let mut s = Sessions::new(0);
        s.apply(&ev("a", Kind::Failed, 0));
        assert_eq!(s.mood(29_000, false, false), Mood::Failed);
        assert_eq!(s.mood(31_000, false, false), Mood::Idle);
        s.apply(&ev("b", Kind::Done, 100_000));
        assert_eq!(s.mood(107_000, false, false), Mood::Done);
        assert_eq!(s.mood(109_000, false, false), Mood::Idle);
    }

    #[test]
    fn panel_and_setup() {
        let mut s = Sessions::new(0);
        assert_eq!(s.mood(1_000, true, false), Mood::Listening);
        s.apply(&ev("a", Kind::Step, 1_500));
        assert_eq!(s.mood(2_000, true, false), Mood::Working);
        assert_eq!(s.mood(2_000, true, true), Mood::Setup);
    }

    #[test]
    fn reply_delta_is_working_with_writing_label() {
        let mut s = Sessions::new(0);
        s.apply(&ev("a", Kind::Step, 1_000));
        let mut d = ev("a", Kind::ReplyDelta, 2_000);
        d.label = None;
        s.apply(&d);
        assert_eq!(s.mood(2_500, false, false), Mood::Working);
        assert_eq!(s.list()[0].label.as_deref(), Some("Writing a reply…"));
    }

    #[test]
    fn list_sorted_and_pruned() {
        let mut s = Sessions::new(0);
        s.apply(&ev("a", Kind::Done, 0));
        s.apply(&ev("b", Kind::Done, 5_000));
        let ids: Vec<String> = s.list().into_iter().map(|x| x.session_id).collect();
        assert_eq!(ids, vec!["b", "a"]);
        s.prune(60 * MIN + 1_000);
        let ids: Vec<String> = s.list().into_iter().map(|x| x.session_id).collect();
        assert_eq!(ids, vec!["b"]);
    }
}
