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
    let description = input
        .get("description")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|d| !d.is_empty());
    let label = match tool {
        "Read" => format!("Reading {}", file()),
        "Edit" | "Write" | "MultiEdit" | "NotebookEdit" => format!("Editing {}", file()),
        "Bash" | "PowerShell" => match description {
            Some(d) => truncate(d, 60),
            None => {
                let cmd = input.get("command").and_then(Value::as_str).unwrap_or("");
                format!("Running {}", truncate(cmd, 40))
            }
        },
        "Grep" | "Glob" => "Searching".to_string(),
        "Agent" | "Task" => match description {
            Some(d) => format!("Subagent: {}", truncate(d, 50)),
            None => "Delegating to a subagent".to_string(),
        },
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

#[derive(Debug, Clone, PartialEq)]
pub enum StreamItem {
    Init { session_id: String, api_key_source: String },
    Pet(PetEvent),
    Overage { resets_at: Option<i64> },
    LimitRejected { resets_at: Option<i64> },
}

pub fn from_stream_line(line: &str, project: &str, now: i64) -> Vec<StreamItem> {
    let Ok(v) = serde_json::from_str::<Value>(line) else {
        return vec![];
    };
    let sid = v.get("session_id").and_then(Value::as_str).unwrap_or("").to_string();
    let ev = |kind: Kind, label: Option<String>, text: Option<String>| {
        StreamItem::Pet(PetEvent {
            session_id: sid.clone(),
            project: project.to_string(),
            source: Source::Ask,
            kind,
            label,
            text,
            at: now,
        })
    };
    let str_of = |key: &str| v.get(key).and_then(Value::as_str);
    match (str_of("type"), str_of("subtype")) {
        (Some("system"), Some("init")) => vec![
            StreamItem::Init {
                session_id: sid.clone(),
                api_key_source: str_of("apiKeySource").unwrap_or("").to_string(),
            },
            ev(Kind::Started, None, None),
        ],
        (Some("system"), Some("permission_denied")) => {
            let tool = str_of("tool_name").unwrap_or("a tool");
            vec![ev(Kind::Blocked, Some(format!("Blocked: {tool}")), None)]
        }
        (Some("system"), Some("api_retry")) => {
            let attempt = v.get("attempt").and_then(Value::as_i64).unwrap_or(0);
            let max = v.get("max_retries").and_then(Value::as_i64).unwrap_or(0);
            vec![ev(Kind::Step, Some(format!("Retrying ({attempt}/{max})…")), None)]
        }
        (Some("rate_limit_event"), _) => {
            let info = v.get("rate_limit_info");
            let field = |k: &str| info.and_then(|i| i.get(k));
            let resets_at = field("resetsAt").and_then(Value::as_i64);
            let mut out = vec![];
            if field("isUsingOverage").and_then(Value::as_bool) == Some(true) {
                out.push(StreamItem::Overage { resets_at });
            }
            if field("status").and_then(Value::as_str) == Some("rejected") {
                out.push(StreamItem::LimitRejected { resets_at });
            }
            out
        }
        (Some("stream_event"), _) => {
            let top_level = v.get("parent_tool_use_id").is_none_or(Value::is_null);
            let delta = v.pointer("/event/delta");
            let is_text = delta.and_then(|d| d.get("type")).and_then(Value::as_str) == Some("text_delta");
            if top_level && is_text {
                let t = delta.and_then(|d| d.get("text")).and_then(Value::as_str).unwrap_or("");
                vec![ev(Kind::ReplyDelta, None, Some(t.to_string()))]
            } else {
                vec![]
            }
        }
        (Some("assistant"), _) => v
            .pointer("/message/content")
            .and_then(Value::as_array)
            .map(|blocks| {
                blocks
                    .iter()
                    .filter(|b| b.get("type").and_then(Value::as_str) == Some("tool_use"))
                    .map(|b| {
                        let name = b.get("name").and_then(Value::as_str).unwrap_or("Tool");
                        ev(Kind::Step, Some(step_label(name, b.get("input").unwrap_or(&NULL))), None)
                    })
                    .collect()
            })
            .unwrap_or_default(),
        (Some("result"), _) => {
            let is_error = v.get("is_error").and_then(Value::as_bool).unwrap_or(false);
            let text = str_of("result").map(str::to_string);
            if is_error {
                vec![ev(Kind::Failed, Some("Something went wrong".to_string()), text)]
            } else {
                vec![ev(Kind::Done, Some("Done".to_string()), text)]
            }
        }
        _ => vec![],
    }
}

pub fn resets_text(resets_at_secs: Option<i64>, now_ms: i64) -> String {
    match resets_at_secs {
        Some(secs) => {
            let mins = ((secs * 1000 - now_ms) / 60_000).max(0);
            format!("Resets in {}h {}m.", mins / 60, mins % 60)
        }
        None => "Try again later.".to_string(),
    }
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
            step_label("PowerShell", &json!({"command": "$S = 1; Get-Thing", "description": "Screenshot the pet window"})),
            "Screenshot the pet window"
        );
        assert_eq!(step_label("Bash", &json!({"command": "ls", "description": "  "})), "Running ls");
        assert_eq!(
            step_label("Agent", &json!({"description": "Find the hook server"})),
            "Subagent: Find the hook server"
        );
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

    #[test]
    fn stream_fixture() {
        let fixture = include_str!("../tests/fixtures/stream_success.ndjson");
        let items: Vec<StreamItem> = fixture
            .lines()
            .flat_map(|l| from_stream_line(l, "C:\\proj", 7))
            .collect();
        let ev = |kind: Kind, label: Option<&str>, text: Option<&str>| {
            StreamItem::Pet(PetEvent {
                session_id: "bf558c52".into(),
                project: "C:\\proj".into(),
                source: Source::Ask,
                kind,
                label: label.map(Into::into),
                text: text.map(Into::into),
                at: 7,
            })
        };
        assert_eq!(
            items,
            vec![
                StreamItem::Init { session_id: "bf558c52".into(), api_key_source: "none".into() },
                ev(Kind::Started, None, None),
                ev(Kind::Step, Some("Reading hello.txt"), None),
                ev(Kind::Step, Some("Editing out.txt"), None),
                ev(Kind::Blocked, Some("Blocked: Write"), None),
                ev(Kind::ReplyDelta, None, Some("ban")),
                ev(Kind::ReplyDelta, None, Some("ana")),
                ev(Kind::Done, Some("Done"), Some("banana")),
            ]
        );
    }

    #[test]
    fn overage_and_rejection() {
        let over = r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed","resetsAt":1790679600,"isUsingOverage":true},"session_id":"s"}"#;
        assert_eq!(from_stream_line(over, "p", 1), vec![StreamItem::Overage { resets_at: Some(1790679600) }]);
        let rejected = r#"{"type":"rate_limit_event","rate_limit_info":{"status":"rejected","resetsAt":1790679600,"isUsingOverage":false},"session_id":"s"}"#;
        assert_eq!(from_stream_line(rejected, "p", 1), vec![StreamItem::LimitRejected { resets_at: Some(1790679600) }]);
        let fine = r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed_warning","isUsingOverage":false},"session_id":"s"}"#;
        assert!(from_stream_line(fine, "p", 1).is_empty());
    }

    #[test]
    fn retry_error_and_noise() {
        let retry = r#"{"type":"system","subtype":"api_retry","attempt":2,"max_retries":10,"retry_delay_ms":500,"error_status":529,"error":"overloaded","session_id":"s"}"#;
        match &from_stream_line(retry, "p", 1)[..] {
            [StreamItem::Pet(e)] => assert_eq!((e.kind, e.label.as_deref()), (Kind::Step, Some("Retrying (2/10)…"))),
            other => panic!("{other:?}"),
        }
        let err = r#"{"type":"result","subtype":"error_during_execution","is_error":true,"result":"Invalid API key","session_id":"s"}"#;
        match &from_stream_line(err, "p", 1)[..] {
            [StreamItem::Pet(e)] => assert_eq!((e.kind, e.text.as_deref()), (Kind::Failed, Some("Invalid API key"))),
            other => panic!("{other:?}"),
        }
        let sub_text = r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"x"}},"parent_tool_use_id":"toolu_9","session_id":"s"}"#;
        assert!(from_stream_line(sub_text, "p", 1).is_empty());
        assert!(from_stream_line("not json", "p", 1).is_empty());
        assert!(from_stream_line(r#"{"type":"system","subtype":"status","status":"requesting"}"#, "p", 1).is_empty());
    }

    #[test]
    fn resets_text_formats() {
        assert_eq!(resets_text(Some(3 * 3600 + 20 * 60), 0), "Resets in 3h 20m.");
        assert_eq!(resets_text(Some(10), 20_000), "Resets in 0h 0m.");
        assert_eq!(resets_text(None, 0), "Try again later.");
    }
}
