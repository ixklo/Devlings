use std::{
    io::{self, BufRead, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
};

use crate::{
    money_guard::{self, AuthVerdict},
    normalize::{self, StreamItem},
};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub struct AskRequest {
    pub bin: PathBuf,
    pub project: String,
    pub prompt: String,
    pub mode_flag: &'static str,
    pub resume: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum KillReason {
    NotSubscription(String),
    Overage { resets_at: Option<i64> },
}

pub fn build_args(req: &AskRequest) -> Vec<String> {
    let mut args: Vec<String> = [
        "-p", "--output-format", "stream-json", "--verbose", "--include-partial-messages",
        "--permission-mode", req.mode_flag, "--permission-prompts", "none",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    if let Some(id) = &req.resume {
        args.push("--resume".into());
        args.push(id.clone());
    }
    args
}

/// A child process that never opens a console window.
pub fn background_command(program: &Path) -> Command {
    #[cfg_attr(not(windows), allow(unused_mut))]
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// A Claude Code child: no console window, and no API-key variables in its environment.
pub fn hidden_command(bin: &Path) -> Command {
    let mut cmd = background_command(bin);
    cmd.env_clear().envs(money_guard::clean_env(std::env::vars()));
    cmd
}

pub fn version_of(bin: &Path) -> Option<String> {
    let out = hidden_command(bin).arg("--version").stdin(Stdio::null()).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).to_string())
}

pub fn auth_status(bin: &Path) -> AuthVerdict {
    match hidden_command(bin).args(["auth", "status"]).stdin(Stdio::null()).output() {
        Ok(out) => money_guard::verdict_from_auth_status(&String::from_utf8_lossy(&out.stdout), out.status.success()),
        Err(e) => AuthVerdict::Refused { reason: format!("Couldn't run Claude Code: {e}") },
    }
}

pub fn spawn(req: &AskRequest) -> io::Result<Child> {
    let mut child = hidden_command(&req.bin)
        .args(build_args(req))
        .current_dir(&req.project)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let mut stdin = child.stdin.take().expect("stdin is piped");
    stdin.write_all(req.prompt.as_bytes())?;
    drop(stdin);
    Ok(child)
}

pub fn pump<R: BufRead>(
    reader: R,
    project: &str,
    now: impl Fn() -> i64,
    mut emit: impl FnMut(StreamItem),
) -> Option<KillReason> {
    for line in reader.lines() {
        let Ok(line) = line else { break };
        for item in normalize::from_stream_line(&line, project, now()) {
            let kill = match &item {
                StreamItem::Init { api_key_source, .. } if !money_guard::init_allowed(api_key_source) => {
                    Some(KillReason::NotSubscription(format!(
                        "Claude Code started with an API key ({api_key_source}) instead of your subscription."
                    )))
                }
                StreamItem::Overage { resets_at } => Some(KillReason::Overage { resets_at: *resets_at }),
                _ => None,
            };
            match kill {
                Some(k @ KillReason::NotSubscription(_)) => return Some(k),
                Some(k) => {
                    emit(item);
                    return Some(k);
                }
                None => emit(item),
            }
        }
    }
    None
}

pub fn kill_tree(pid: u32) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = Command::new("taskkill")
            .creation_flags(CREATE_NO_WINDOW)
            .args(["/T", "/F", "/PID", &pid.to_string()])
            .status();
    }
    #[cfg(not(windows))]
    {
        let _ = Command::new("kill").args(["-INT", &pid.to_string()]).status();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::Kind;
    use std::io::Cursor;

    fn req(resume: Option<&str>) -> AskRequest {
        AskRequest {
            bin: "claude".into(),
            project: "C:\\proj".into(),
            prompt: "hi".into(),
            mode_flag: "acceptEdits",
            resume: resume.map(Into::into),
        }
    }

    #[test]
    fn args() {
        assert_eq!(
            build_args(&req(None)),
            vec![
                "-p", "--output-format", "stream-json", "--verbose", "--include-partial-messages",
                "--permission-mode", "acceptEdits", "--permission-prompts", "none"
            ]
        );
        assert_eq!(build_args(&req(Some("abc")))[9..].to_vec(), vec!["--resume", "abc"]);
    }

    #[test]
    fn pump_passes_items_through() {
        let fixture = include_str!("../tests/fixtures/stream_success.ndjson");
        let mut items = vec![];
        let kill = pump(Cursor::new(fixture), "C:\\proj", || 1, |i| items.push(i));
        assert_eq!(kill, None);
        assert_eq!(items.len(), 8);
        assert!(matches!(items.last(), Some(StreamItem::Pet(e)) if e.kind == Kind::Done));
    }

    #[test]
    fn pump_kills_on_api_key_init() {
        let input = concat!(
            r#"{"type":"system","subtype":"init","session_id":"s","apiKeySource":"ANTHROPIC_API_KEY"}"#, "\n",
            r#"{"type":"result","is_error":false,"result":"x","session_id":"s"}"#, "\n",
        );
        let mut items = vec![];
        let kill = pump(Cursor::new(input), "p", || 1, |i| items.push(i));
        assert!(matches!(kill, Some(KillReason::NotSubscription(_))));
        assert!(items.is_empty());
    }

    #[test]
    fn pump_kills_on_overage() {
        let input = concat!(
            r#"{"type":"system","subtype":"init","session_id":"s","apiKeySource":"none"}"#, "\n",
            r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed","isUsingOverage":true,"resetsAt":100},"session_id":"s"}"#, "\n",
            r#"{"type":"result","is_error":false,"result":"x","session_id":"s"}"#, "\n",
        );
        let mut items = vec![];
        let kill = pump(Cursor::new(input), "p", || 1, |i| items.push(i));
        assert_eq!(kill, Some(KillReason::Overage { resets_at: Some(100) }));
        assert_eq!(items.len(), 3);
        assert_eq!(items[2], StreamItem::Overage { resets_at: Some(100) });
    }
}
