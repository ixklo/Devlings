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
            Some("permission_prompt") => (Kind::NeedsYou, Some(NEEDS_APPROVAL.to_string()), None),
            _ => return None,
        },
        // Arrives the moment Claude Code shows its own prompt (design v1.0 D5).
        "PermissionRequest" => (Kind::NeedsYou, Some(NEEDS_APPROVAL.to_string()), None),
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

/// A tool that ran (PostToolUse, PostToolUseFailure) or was denied (PermissionDenied), as the update that puts
/// a thread waiting on a permission prompt back to work. The caller applies it only to a waiting thread.
pub fn from_hook_after_tool(body: &Value, now: i64) -> Option<PetEvent> {
    let session_id = body.get("session_id")?.as_str()?.to_string();
    let project = body.get("cwd").and_then(Value::as_str).unwrap_or("").to_string();
    let tool = body.get("tool_name").and_then(Value::as_str).unwrap_or("Tool");
    let (kind, label) = match body.get("hook_event_name")?.as_str()? {
        "PostToolUse" | "PostToolUseFailure" => (Kind::Step, step_label(tool, body.get("tool_input").unwrap_or(&NULL))),
        "PermissionDenied" => (Kind::Blocked, format!("Blocked: {tool}")),
        _ => return None,
    };
    Some(PetEvent { session_id, project, source: Source::Watch, kind, label: Some(label), text: None, at: now })
}

pub const NEEDS_APPROVAL: &str = "Needs your approval";

#[derive(Debug, Clone, PartialEq)]
pub enum StreamItem {
    Init { session_id: String, api_key_source: String },
    Pet(PetEvent),
    Overage { resets_at: Option<i64> },
    LimitRejected { resets_at: Option<i64> },
    /// Host protocol: Claude Code asks whether a tool may run. `request` is the control request's body.
    CanUseTool { request_id: String, request: Value },
    /// Host protocol: a control request Perch doesn't handle.
    ControlRequest { request_id: String, subtype: String },
    /// Host protocol: Claude Code withdrew a request (for example, a hook allowed the tool first).
    ControlCancel { request_id: String },
}

pub fn from_stream_line(line: &str, project: &str, now: i64) -> Vec<StreamItem> {
    let line = line.trim_start_matches('\u{feff}');
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
        (Some("control_request"), _) => {
            let Some(request_id) = str_of("request_id").map(str::to_string) else { return vec![] };
            let request = v.get("request").cloned().unwrap_or(Value::Null);
            match request.get("subtype").and_then(Value::as_str) {
                Some("can_use_tool") => vec![StreamItem::CanUseTool { request_id, request }],
                other => vec![StreamItem::ControlRequest { request_id, subtype: other.unwrap_or("").to_string() }],
            }
        }
        (Some("control_cancel_request"), _) => match str_of("request_id") {
            Some(id) => vec![StreamItem::ControlCancel { request_id: id.to_string() }],
            None => vec![],
        },
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

    fn kinds_and_labels(items: &[StreamItem]) -> Vec<(String, Option<String>)> {
        items
            .iter()
            .map(|i| match i {
                StreamItem::Init { api_key_source, .. } => (format!("init:{api_key_source}"), None),
                StreamItem::Pet(e) => (format!("{:?}", e.kind), e.label.clone()),
                other => (format!("{other:?}"), None),
            })
            .collect()
    }

    fn stream(fixture: &str) -> Vec<StreamItem> {
        fixture.lines().flat_map(|l| from_stream_line(l, "C:\\proj", 1)).collect()
    }

    fn label(kind: &str, label: Option<&str>) -> (String, Option<String>) {
        (kind.to_string(), label.map(str::to_string))
    }

    /// Claude Code 2.1.282: a PermissionRequest hook denied two tools. Captured through PowerShell, so the
    /// file starts with a byte order mark.
    #[test]
    fn fixture_2_1_282_hook_deny() {
        let items = stream(include_str!("../tests/fixtures/cc2.1.282_stream_hook_deny.ndjson"));
        assert_eq!(
            kinds_and_labels(&items),
            vec![
                label("init:none", None),
                label("Started", None),
                label("Step", Some("Write \"hi\" to out2.txt file")),
                label("Blocked", Some("Blocked: Bash")),
                label("Step", Some("Write \"hi\" to out2.txt file")),
                label("Blocked", Some("Blocked: PowerShell")),
                label("Done", Some("Done")),
            ]
        );
        match items.last() {
            Some(StreamItem::Pet(e)) => {
                assert_eq!((e.text.as_deref(), e.session_id.as_str()), (Some("Unable."), "ff7af3e7-aacb-4627-aa74-6c16eb5bb108"))
            }
            other => panic!("{other:?}"),
        }
    }

    /// Claude Code 2.1.282: an HTTP Stop hook with Perch not running adds a `system/notification`, which is ignored.
    #[test]
    fn fixture_2_1_282_stop_hook_error() {
        let items = stream(include_str!("../tests/fixtures/cc2.1.282_stream_stop_hook_error.ndjson"));
        assert_eq!(
            kinds_and_labels(&items),
            vec![label("init:none", None), label("Started", None), label("Step", Some("Reading hello.txt")), label("Done", Some("Done"))]
        );
    }

    /// Claude Code 2.1.282: no hook answer in a `-p` run, so the tool is denied ("no approval surface").
    #[test]
    fn fixture_2_1_282_no_approval_surface() {
        let items = stream(include_str!("../tests/fixtures/cc2.1.282_stream_no_approval.ndjson"));
        assert_eq!(
            kinds_and_labels(&items),
            vec![
                label("init:none", None),
                label("Started", None),
                label("Step", Some("Editing out4.txt")),
                label("Blocked", Some("Blocked: Write")),
                label("Done", Some("Done")),
            ]
        );
    }

    /// Hook bodies from Claude Code 2.1.282. The UserPromptSubmit body was captured; the PermissionRequest body
    /// is rebuilt from the same session and the key list recorded in the v1.0 design, section 2.
    #[test]
    fn fixture_2_1_282_hook_bodies() {
        let bodies: Vec<Value> = include_str!("../tests/fixtures/cc2.1.282_hook_bodies.jsonl")
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        let prompt = from_hook(&bodies[0], 3).unwrap();
        assert_eq!((prompt.kind, prompt.project.as_str(), prompt.source), (Kind::Prompt, "C:\\Users\\me\\proj", Source::Watch));
        assert_eq!(bodies[1]["hook_event_name"], "PermissionRequest");
        // A permission request shows as "needs input" at once (design D5).
        let needs = from_hook(&bodies[1], 3).unwrap();
        assert_eq!((needs.kind, needs.label.as_deref()), (Kind::NeedsYou, Some("Needs your approval")));
    }

    #[test]
    fn tools_that_ran_or_were_denied() {
        let ran = from_hook_after_tool(&hook("PostToolUse", json!({"tool_name": "Bash", "tool_input": {"command": "npm test"}})), 4).unwrap();
        assert_eq!((ran.kind, ran.label.as_deref(), ran.at, ran.source), (Kind::Step, Some("Running npm test"), 4, Source::Watch));
        let failed = from_hook_after_tool(&hook("PostToolUseFailure", json!({"tool_name": "Read", "tool_input": {"file_path": "C:\\a\\b.rs"}})), 4).unwrap();
        assert_eq!((failed.kind, failed.label.as_deref()), (Kind::Step, Some("Reading b.rs")));
        let denied = from_hook_after_tool(&hook("PermissionDenied", json!({"tool_name": "Write"})), 4).unwrap();
        assert_eq!((denied.kind, denied.label.as_deref()), (Kind::Blocked, Some("Blocked: Write")));
        assert!(from_hook_after_tool(&hook("PreToolUse", json!({"tool_name": "Bash"})), 4).is_none());
        assert!(from_hook_after_tool(&json!({"hook_event_name": "PostToolUse"}), 4).is_none());
        // These never change a thread on their own.
        assert!(from_hook(&hook("PostToolUse", json!({})), 1).is_none());
        assert!(from_hook(&hook("PermissionDenied", json!({})), 1).is_none());
    }

    /// Claude Code 2.1.282 with the host protocol: a `can_use_tool` request, then its withdrawal.
    #[test]
    fn fixture_2_1_282_host_protocol() {
        let items = stream(include_str!("../tests/fixtures/cc2.1.282_host_cancel.ndjson"));
        let ask = items.iter().find_map(|i| match i {
            StreamItem::CanUseTool { request_id, request } => Some((request_id.clone(), request.clone())),
            _ => None,
        });
        let (id, request) = ask.unwrap();
        assert_eq!(id, "8b55f4ca-71a0-4681-a59f-7763dd84e624");
        assert_eq!((request["subtype"].as_str(), request["tool_name"].as_str()), (Some("can_use_tool"), Some("PowerShell")));
        assert!(items.contains(&StreamItem::ControlCancel { request_id: id }));
        assert!(matches!(items.last(), Some(StreamItem::Pet(e)) if e.kind == Kind::Done));
        let other = r#"{"type":"control_request","request_id":"q","request":{"subtype":"mcp_message"}}"#;
        assert_eq!(from_stream_line(other, "p", 1), vec![StreamItem::ControlRequest { request_id: "q".into(), subtype: "mcp_message".into() }]);
        assert!(from_stream_line(r#"{"type":"control_request","request":{"subtype":"can_use_tool"}}"#, "p", 1).is_empty());
        assert!(from_stream_line(r#"{"type":"control_cancel_request"}"#, "p", 1).is_empty());
        assert!(from_stream_line(r#"{"type":"control_response","response":{}}"#, "p", 1).is_empty());
    }

    /// Unknown events, unknown fields and wrong types are ignored, never a panic.
    #[test]
    fn odd_input_is_ignored() {
        assert!(from_hook(&json!({"hook_event_name": "Elicitation", "session_id": "s"}), 1).is_none());
        assert!(from_hook(&json!({"hook_event_name": 5, "session_id": "s"}), 1).is_none());
        assert!(from_hook(&json!({"hook_event_name": "Stop", "session_id": 5}), 1).is_none());
        assert!(from_hook(&json!([1, 2]), 1).is_none());
        assert!(from_hook(&Value::Null, 1).is_none());
        let odd = from_hook(
            &json!({"hook_event_name": "PreToolUse", "session_id": "s", "tool_name": 3, "tool_input": "x", "extra": {"a": 1}}),
            1,
        )
        .unwrap();
        assert_eq!(odd.label.as_deref(), Some("Tool"));
        let stop =
            from_hook(&json!({"hook_event_name": "Stop", "session_id": "s", "cwd": 7, "last_assistant_message": ["x"]}), 1).unwrap();
        assert_eq!((stop.project.as_str(), stop.text), ("", None));
        let odd_lines = [
            "",
            "{",
            "[]",
            "null",
            "\u{feff}",
            r#"{"type":"assistant","message":{"content":"text"}}"#,
            r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":7}]}}"#,
            r#"{"type":"rate_limit_event","rate_limit_info":"x"}"#,
            r#"{"type":"stream_event","event":{"delta":"x"}}"#,
            r#"{"type":"result","is_error":"yes","result":5}"#,
            r#"{"type":"system","subtype":"brand_new_event","x":1}"#,
        ];
        for line in odd_lines {
            let _ = from_stream_line(line, "p", 1);
        }
        // A byte order mark in front of a line doesn't hide it.
        let init = format!("\u{feff}{}", r#"{"type":"system","subtype":"init","session_id":"s","apiKeySource":"none"}"#);
        assert_eq!(from_stream_line(&init, "p", 1).len(), 2);
    }

    #[test]
    fn resets_text_formats() {
        assert_eq!(resets_text(Some(3 * 3600 + 20 * 60), 0), "Resets in 3h 20m.");
        assert_eq!(resets_text(Some(10), 20_000), "Resets in 0h 0m.");
        assert_eq!(resets_text(None, 0), "Try again later.");
    }
}
