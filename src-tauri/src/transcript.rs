use std::path::Path;

use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChatTurn {
    pub role: String,
    pub text: String,
}

pub fn parse_transcript(content: &str) -> Vec<ChatTurn> {
    let mut turns: Vec<ChatTurn> = Vec::new();
    for line in content.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
        let flag = |k: &str| v.get(k).and_then(Value::as_bool) == Some(true);
        if flag("isSidechain") || flag("isMeta") {
            continue;
        }
        let role = match v.get("type").and_then(Value::as_str) {
            Some("user") => "user",
            Some("assistant") => "assistant",
            _ => continue,
        };
        let text = match v.pointer("/message/content") {
            Some(Value::String(s)) => s.clone(),
            Some(Value::Array(blocks)) => blocks
                .iter()
                .filter(|b| b.get("type").and_then(Value::as_str) == Some("text"))
                .filter_map(|b| b.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("\n\n"),
            _ => continue,
        };
        let text = text.trim().to_string();
        let command_echo = role == "user" && (text.starts_with("<command-") || text.starts_with("<local-command"));
        if text.is_empty() || command_echo {
            continue;
        }
        match turns.last_mut() {
            Some(last) if role == "assistant" && last.role == "assistant" => {
                last.text.push_str("\n\n");
                last.text.push_str(&text);
            }
            _ => turns.push(ChatTurn { role: role.to_string(), text }),
        }
    }
    turns
}

pub fn load(path: &Path) -> Vec<ChatTurn> {
    std::fs::read_to_string(path).map(|c| parse_transcript(&c)).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_only_visible_turns() {
        let content = [
            r#"{"type":"queue-operation"}"#,
            r#"{"type":"user","message":{"role":"user","content":"Fix the tests"},"isSidechain":false}"#,
            r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"<system-reminder>x</system-reminder>"}]},"isMeta":true}"#,
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"thinking","thinking":"hmm"}]}}"#,
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Looking now."}]}}"#,
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"t1","name":"Read","input":{}}]}}"#,
            r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"t1","content":"..."}]}}"#,
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"All green."}]}}"#,
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"sub"}]},"isSidechain":true}"#,
            r#"{"type":"user","message":{"role":"user","content":"<command-name>/model</command-name>"}}"#,
            "not json",
        ]
        .join("\n");
        assert_eq!(
            parse_transcript(&content),
            vec![
                ChatTurn { role: "user".into(), text: "Fix the tests".into() },
                ChatTurn { role: "assistant".into(), text: "Looking now.\n\nAll green.".into() },
            ]
        );
    }

    #[test]
    fn missing_file_is_empty() {
        assert!(load(std::path::Path::new("Z:\\definitely\\missing.jsonl")).is_empty());
    }
}
