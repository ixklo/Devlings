use serde_json::Value;

use crate::events::{Kind, PetEvent, Source};

pub static NULL: Value = Value::Null;

pub fn step_label(tool: &str, input: &Value) -> String {
    let file = || {
        input
            .get("file_path")
            .and_then(Value::as_str)
            .map(file_name)
            .unwrap_or_default()
    };
    let label = match tool {
        "Read" => format!("Reading {}", file()),
        "Edit" | "Write" | "MultiEdit" | "NotebookEdit" => format!("Editing {}", file()),
        "Bash" | "PowerShell" => {
            let cmd = input.get("command").and_then(Value::as_str).unwrap_or("");
            format!("Running {}", truncate(cmd, 40))
        }
        "Grep" | "Glob" => "Searching".to_string(),
        "Agent" | "Task" => "Delegating to a subagent".to_string(),
        other => other.to_string(),
    };
    label.trim_end().to_string()
}

fn file_name(path: &str) -> String {
    path.rsplit(['/', '\\']).next().unwrap_or(path).to_string()
}

fn truncate(s: &str, max: usize) -> String {
    let first = s.lines().next().unwrap_or("");
    if first.chars().count() <= max {
        first.to_string()
    } else {
        format!("{}…", first.chars().take(max).collect::<String>())
    }
}

fn excerpt(s: &str, max: usize) -> String {
    s.chars().take(max).collect::<String>().trim().to_string()
}

pub fn from_hook(body: &Value, now: i64) -> Option<PetEvent> {
    let session_id = body.get("session_id")?.as_str()?.to_string();
    let project = body.get("cwd").and_then(Value::as_str).unwrap_or("").to_string();
    let name = body.get("hook_event_name")?.as_str()?;
    let (kind, label, text) = match name {
        "SessionStart" => (Kind::Started, None, None),
        "UserPromptSubmit" => (Kind::Prompt, Some("Thinking…".to_string()), None),
        "PreToolUse" => {
            let tool = body.get("tool_name").and_then(Value::as_str).unwrap_or("Tool");
            let input = body.get("tool_input").unwrap_or(&NULL);
            (Kind::Step, Some(step_label(tool, input)), None)
        }
        "Notification" => match body.get("notification_type").and_then(Value::as_str) {
            Some("permission_prompt") => (Kind::NeedsYou, Some("Needs your approval".to_string()), None),
            _ => return None,
        },
        "Stop" => (
            Kind::Done,
            Some("Done".to_string()),
            body.get("last_assistant_message")
                .and_then(Value::as_str)
                .map(|s| excerpt(s, 200)),
        ),
        "StopFailure" => (Kind::Failed, Some("Something went wrong".to_string()), None),
        "SessionEnd" => (Kind::Ended, None, None),
        _ => return None,
    };
    Some(PetEvent { session_id, project, source: Source::Watch, kind, label, text, at: now })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{Kind, PetEvent, Source};
    use serde_json::json;

    #[test]
    fn step_labels() {
        assert_eq!(step_label("Read", &json!({"file_path": "C:\\proj\\src\\app.tsx"})), "Reading app.tsx");
        assert_eq!(step_label("Edit", &json!({"file_path": "/home/u/proj/main.rs"})), "Editing main.rs");
        assert_eq!(step_label("Write", &json!({"file_path": "out.txt"})), "Editing out.txt");
        assert_eq!(step_label("Bash", &json!({"command": "npm test"})), "Running npm test");
        assert_eq!(
            step_label("Bash", &json!({"command": "a".repeat(60)})),
            format!("Running {}…", "a".repeat(40))
        );
        assert_eq!(step_label("Bash", &json!({"command": "line1\nline2"})), "Running line1");
        assert_eq!(step_label("Grep", &json!({"pattern": "x"})), "Searching");
        assert_eq!(step_label("Glob", &json!({})), "Searching");
        assert_eq!(step_label("Agent", &json!({})), "Delegating to a subagent");
        assert_eq!(step_label("WebFetch", &json!({})), "WebFetch");
        assert_eq!(step_label("Read", &json!({})), "Reading");
    }

    fn hook(name: &str, extra: serde_json::Value) -> serde_json::Value {
        let mut v = json!({
            "session_id": "s1",
            "cwd": "C:\\proj",
            "hook_event_name": name,
            "transcript_path": "C:\\t.jsonl"
        });
        for (k, val) in extra.as_object().unwrap() {
            v[k] = val.clone();
        }
        v
    }

    #[test]
    fn hook_mapping() {
        let e = from_hook(
            &hook("PreToolUse", json!({"tool_name": "Read", "tool_input": {"file_path": "a/b.rs"}})),
            5,
        )
        .unwrap();
        assert_eq!(
            e,
            PetEvent {
                session_id: "s1".into(),
                project: "C:\\proj".into(),
                source: Source::Watch,
                kind: Kind::Step,
                label: Some("Reading b.rs".into()),
                text: None,
                at: 5
            }
        );
        assert_eq!(from_hook(&hook("SessionStart", json!({})), 1).unwrap().kind, Kind::Started);
        assert_eq!(from_hook(&hook("UserPromptSubmit", json!({"prompt": "hi"})), 1).unwrap().kind, Kind::Prompt);
        let n = from_hook(&hook("Notification", json!({"notification_type": "permission_prompt"})), 1).unwrap();
        assert_eq!((n.kind, n.label.as_deref()), (Kind::NeedsYou, Some("Needs your approval")));
        assert!(from_hook(&hook("Notification", json!({"notification_type": "idle_prompt"})), 1).is_none());
        let d = from_hook(&hook("Stop", json!({"last_assistant_message": "All tests pass."})), 1).unwrap();
        assert_eq!((d.kind, d.text.as_deref()), (Kind::Done, Some("All tests pass.")));
        assert_eq!(from_hook(&hook("StopFailure", json!({})), 1).unwrap().kind, Kind::Failed);
        assert_eq!(from_hook(&hook("SessionEnd", json!({})), 1).unwrap().kind, Kind::Ended);
        assert!(from_hook(&hook("PostToolUse", json!({})), 1).is_none());
        assert!(from_hook(&json!({"hook_event_name": "Stop"}), 1).is_none());
    }

    #[test]
    fn serializes_for_frontend() {
        let e = from_hook(&hook("Notification", json!({"notification_type": "permission_prompt"})), 9).unwrap();
        let s = serde_json::to_value(&e).unwrap();
        assert_eq!(s["sessionId"], "s1");
        assert_eq!(s["kind"], "needs_you");
        assert_eq!(s["source"], "watch");
        assert!(s.get("text").is_none());
    }
}
