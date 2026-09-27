use std::{
    io::{self, BufRead, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::mpsc,
    thread::JoinHandle,
};

use serde_json::{json, Value};

use crate::{
    events::Kind,
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
    /// Appended after every other flag, e.g. `--setting-sources user` for an untrusted folder (see `trust`).
    pub extra_args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum KillReason {
    NotSubscription(String),
    Overage { resets_at: Option<i64> },
}

/// Ask runs speak Claude Code's host protocol (design v1.0 D4): the prompt goes in as a stream-json user message,
/// and permission prompts come back as `can_use_tool` control requests that Perch answers on stdin.
pub fn build_args(req: &AskRequest) -> Vec<String> {
    let mut args: Vec<String> = [
        "-p", "--output-format", "stream-json", "--verbose", "--include-partial-messages",
        "--input-format", "stream-json", "--permission-prompt-tool", "stdio",
        "--permission-mode", req.mode_flag,
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    if let Some(id) = &req.resume {
        args.push("--resume".into());
        args.push(id.clone());
    }
    args.extend(req.extra_args.iter().cloned());
    args
}

/// The prompt as one stream-json line: `{"type":"user","message":{"role":"user","content":"…"}}`.
pub fn user_message_line(prompt: &str) -> String {
    json!({ "type": "user", "message": { "role": "user", "content": prompt } }).to_string()
}

/// A successful answer to a control request.
pub fn control_response(request_id: &str, response: Value) -> String {
    json!({ "type": "control_response", "response": { "subtype": "success", "request_id": request_id, "response": response } })
        .to_string()
}

/// Refuses a control request Perch doesn't handle, so Claude Code never waits on it.
pub fn control_error(request_id: &str, error: &str) -> String {
    json!({ "type": "control_response", "response": { "subtype": "error", "request_id": request_id, "error": error } }).to_string()
}

/// An Ask process's stdin, kept open behind a writer thread so the pump and approval answers can both write
/// lines to it without blocking. Stdin closes on `close()`, or once every clone is dropped.
#[derive(Clone)]
pub struct StdinWriter {
    tx: mpsc::Sender<Option<String>>,
}

impl StdinWriter {
    pub fn spawn<W: Write + Send + 'static>(mut stdin: W) -> (Self, JoinHandle<()>) {
        let (tx, rx) = mpsc::channel::<Option<String>>();
        let handle = std::thread::spawn(move || {
            for line in rx.iter().map_while(|m| m) {
                if writeln!(stdin, "{line}").and_then(|_| stdin.flush()).is_err() {
                    break;
                }
            }
        });
        (Self { tx }, handle)
    }

    /// Queues one line (a newline is added). False when stdin is already closed.
    pub fn send(&self, line: String) -> bool {
        self.tx.send(Some(line)).is_ok()
    }

    pub fn close(&self) {
        let _ = self.tx.send(None);
    }
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

/// Starts an Ask run and sends its prompt. Stdin stays open behind the returned writer for permission answers.
pub fn spawn(req: &AskRequest) -> io::Result<(Child, StdinWriter)> {
    let mut child = hidden_command(&req.bin)
        .args(build_args(req))
        .current_dir(&req.project)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let Some(stdin) = child.stdin.take() else {
        kill_tree(child.id());
        let _ = child.wait();
        return Err(io::Error::other("Claude Code's input wasn't captured"));
    };
    let (writer, _) = StdinWriter::spawn(stdin);
    writer.send(user_message_line(&req.prompt));
    Ok((child, writer))
}

/// Reads an Ask run's stream and hands each item to `emit`, speaking the host protocol on `stdin`:
/// - `can_use_tool` requests and their withdrawals are passed on (the caller registers and answers them);
/// - any other control request is refused at once;
/// - the result closes stdin, so Claude Code exits.
///
/// Returns why the money guard stopped the run, if it did; stdin is closed then too.
pub fn pump<R: BufRead>(
    reader: R,
    project: &str,
    now: impl Fn() -> i64,
    stdin: &StdinWriter,
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
                Some(k @ KillReason::NotSubscription(_)) => {
                    stdin.close();
                    return Some(k);
                }
                Some(k) => {
                    emit(item);
                    stdin.close();
                    return Some(k);
                }
                None => {}
            }
            match item {
                StreamItem::ControlRequest { request_id, subtype } => {
                    log::debug!("Refused an unsupported control request ({subtype}) from Claude Code");
                    stdin.send(control_error(&request_id, "unsupported"));
                }
                StreamItem::Pet(ev) if matches!(ev.kind, Kind::Done | Kind::Failed) => {
                    emit(StreamItem::Pet(ev));
                    stdin.close();
                }
                other => emit(other),
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

/// A stand-in for an Ask process's stdin, for tests.
#[cfg(test)]
pub mod fake_stdin {
    use super::StdinWriter;
    use std::{
        io::Write,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc, Mutex,
        },
        thread::JoinHandle,
        time::{Duration, Instant},
    };

    /// What was written, and whether the pipe was closed.
    #[derive(Clone, Default)]
    pub struct FakeStdin {
        bytes: Arc<Mutex<Vec<u8>>>,
        closed: Arc<AtomicBool>,
    }

    struct Pipe(FakeStdin);

    impl Write for Pipe {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.bytes.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl Drop for Pipe {
        fn drop(&mut self) {
            self.0.closed.store(true, Ordering::SeqCst);
        }
    }

    impl FakeStdin {
        pub fn text(&self) -> String {
            String::from_utf8(self.bytes.lock().unwrap().clone()).unwrap()
        }

        pub fn lines(&self) -> Vec<serde_json::Value> {
            self.text().lines().map(|l| serde_json::from_str(l).unwrap()).collect()
        }

        pub fn is_closed(&self) -> bool {
            self.closed.load(Ordering::SeqCst)
        }

        /// Waits up to 5 s for the pipe to close.
        pub fn wait_closed(&self) -> bool {
            let t0 = Instant::now();
            while !self.is_closed() && t0.elapsed() < Duration::from_secs(5) {
                std::thread::sleep(Duration::from_millis(5));
            }
            self.is_closed()
        }
    }

    pub fn writer() -> (StdinWriter, FakeStdin, JoinHandle<()>) {
        let fake = FakeStdin::default();
        let (writer, join) = StdinWriter::spawn(Pipe(fake.clone()));
        (writer, fake, join)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn req(resume: Option<&str>) -> AskRequest {
        AskRequest {
            bin: "claude".into(),
            project: "C:\\proj".into(),
            prompt: "hi".into(),
            mode_flag: "acceptEdits",
            resume: resume.map(Into::into),
            extra_args: vec![],
        }
    }

    #[test]
    fn args() {
        assert_eq!(
            build_args(&req(None)),
            vec![
                "-p", "--output-format", "stream-json", "--verbose", "--include-partial-messages",
                "--input-format", "stream-json", "--permission-prompt-tool", "stdio",
                "--permission-mode", "acceptEdits"
            ]
        );
        assert_eq!(build_args(&req(Some("abc")))[11..].to_vec(), vec!["--resume", "abc"]);
        // Permission prompts come to Perch over stdin now; nothing turns them off.
        assert!(!build_args(&req(None)).contains(&"--permission-prompts".to_string()));
        // An untrusted folder's extra flags still come after everything, host protocol included.
        let mut untrusted = req(Some("abc"));
        untrusted.extra_args = vec!["--setting-sources".into(), "user".into()];
        assert_eq!(
            build_args(&untrusted),
            vec![
                "-p", "--output-format", "stream-json", "--verbose", "--include-partial-messages",
                "--input-format", "stream-json", "--permission-prompt-tool", "stdio",
                "--permission-mode", "acceptEdits", "--resume", "abc", "--setting-sources", "user"
            ]
        );
    }

    #[test]
    fn the_prompt_is_one_json_user_message_line() {
        let prompt = "Say \"hi\"\nthen\tstop \\ now: caf\u{e9} \u{1f426} \u{2028}";
        let line = user_message_line(prompt);
        assert!(!line.contains('\n') && !line.contains('\r'), "{line}");
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v, serde_json::json!({"type": "user", "message": {"role": "user", "content": prompt}}));
        assert_eq!(user_message_line(""), r#"{"type":"user","message":{"role":"user","content":""}}"#);
    }

    #[test]
    fn control_lines() {
        let ok: serde_json::Value = serde_json::from_str(&control_response("r1", serde_json::json!({"behavior": "allow"}))).unwrap();
        assert_eq!(
            ok,
            serde_json::json!({"type": "control_response", "response": {"subtype": "success", "request_id": "r1", "response": {"behavior": "allow"}}})
        );
        let err: serde_json::Value = serde_json::from_str(&control_error("r2", "unsupported")).unwrap();
        assert_eq!(err, serde_json::json!({"type": "control_response", "response": {"subtype": "error", "request_id": "r2", "error": "unsupported"}}));
    }

    #[test]
    fn stdin_writer_writes_lines_and_closes() {
        let (stdin, fake, join) = fake_stdin::writer();
        assert!(stdin.send("one".into()));
        let clone = stdin.clone();
        assert!(clone.send("two".into()));
        clone.close();
        join.join().unwrap();
        assert_eq!(fake.text(), "one\ntwo\n");
        assert!(fake.is_closed());
        // After closing, lines go nowhere and nothing panics.
        assert!(!stdin.send("three".into()));
        stdin.close();
    }

    #[test]
    fn stdin_closes_when_every_writer_is_gone() {
        let (stdin, fake, join) = fake_stdin::writer();
        let clone = stdin.clone();
        drop(stdin);
        assert!(!fake.is_closed());
        drop(clone);
        join.join().unwrap();
        assert!(fake.is_closed());
    }

    fn run(fixture: &str) -> (Option<KillReason>, Vec<StreamItem>, fake_stdin::FakeStdin, StdinWriter) {
        let (stdin, fake, _join) = fake_stdin::writer();
        let mut items = vec![];
        let kill = pump(Cursor::new(fixture), "C:\\proj", || 1, &stdin, |i| items.push(i));
        (kill, items, fake, stdin)
    }

    #[test]
    fn extra_args_come_after_every_existing_flag() {
        let plain = build_args(&req(Some("abc")));
        assert!(!plain.iter().any(|a| a == "--setting-sources"));
        let mut untrusted = req(Some("abc"));
        untrusted.extra_args = crate::trust::SETTING_SOURCES_USER.iter().map(|s| s.to_string()).collect();
        let args = build_args(&untrusted);
        assert_eq!(args[..plain.len()], plain[..], "every existing flag is still there, in order");
        assert_eq!(args[plain.len()..].to_vec(), vec!["--setting-sources", "user"]);
    }

    #[test]
    fn pump_passes_items_through() {
        let (kill, items, fake, _stdin) = run(include_str!("../tests/fixtures/stream_success.ndjson"));
        assert_eq!(kill, None);
        assert_eq!(items.len(), 8);
        assert!(matches!(items.last(), Some(StreamItem::Pet(e)) if e.kind == Kind::Done));
        // The result ends the conversation, so stdin is closed and Claude Code exits.
        assert!(fake.wait_closed());
    }

    /// Claude Code 2.1.282 asked the host about a tool, and the host denied it.
    #[test]
    fn pump_hands_over_permission_requests_and_answers_go_to_stdin() {
        let (stdin, fake, _join) = fake_stdin::writer();
        let mut requests = vec![];
        let fixture = include_str!("../tests/fixtures/cc2.1.282_host_deny.ndjson");
        let kill = pump(Cursor::new(fixture), "C:\\proj", || 1, &stdin, |i| {
            if let StreamItem::CanUseTool { request_id, request } = &i {
                assert!(!fake.is_closed(), "stdin must stay open while a request waits");
                // What `answer_approval` does when the user clicks Deny.
                stdin.send(control_response(request_id, serde_json::json!({"behavior": "deny", "message": "The user declined this in Perch."})));
                requests.push((request_id.clone(), request.clone()));
            }
        });
        assert_eq!(kill, None);
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].0, "8de93467-bf47-45db-85b1-34b4905e586e");
        assert_eq!(requests[0].1["tool_name"], "PowerShell");
        assert!(fake.wait_closed());
        let lines = fake.lines();
        assert_eq!(lines.len(), 1);
        assert_eq!(
            lines[0],
            serde_json::json!({"type": "control_response", "response": {"subtype": "success", "request_id": "8de93467-bf47-45db-85b1-34b4905e586e", "response": {"behavior": "deny", "message": "The user declined this in Perch."}}})
        );
    }

    /// Claude Code 2.1.282 asked the host, then a hook allowed the tool first and the request was withdrawn.
    #[test]
    fn pump_reports_withdrawn_requests() {
        let (kill, items, fake, _stdin) = run(include_str!("../tests/fixtures/cc2.1.282_host_cancel.ndjson"));
        assert_eq!(kill, None);
        let control: Vec<String> = items
            .iter()
            .filter_map(|i| match i {
                StreamItem::CanUseTool { request_id, .. } => Some(format!("ask {request_id}")),
                StreamItem::ControlCancel { request_id } => Some(format!("cancel {request_id}")),
                _ => None,
            })
            .collect();
        assert_eq!(control, vec!["ask 8b55f4ca-71a0-4681-a59f-7763dd84e624", "cancel 8b55f4ca-71a0-4681-a59f-7763dd84e624"]);
        assert!(fake.wait_closed());
        assert!(fake.lines().is_empty(), "nothing is answered for a withdrawn request");
    }

    #[test]
    fn pump_refuses_unknown_control_requests_so_the_cli_never_hangs() {
        let input = concat!(
            r#"{"type":"system","subtype":"init","session_id":"s","apiKeySource":"none"}"#, "\n",
            r#"{"type":"control_request","request_id":"x1","request":{"subtype":"hook_callback","callback_id":"c"}}"#, "\n",
            r#"{"type":"control_request","request_id":"x2","request":{}}"#, "\n",
        );
        let (kill, items, fake, stdin) = run(input);
        assert_eq!(kill, None);
        assert!(items.iter().all(|i| !matches!(i, StreamItem::CanUseTool { .. } | StreamItem::ControlRequest { .. })));
        stdin.close();
        assert!(fake.wait_closed());
        let lines = fake.lines();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], serde_json::json!({"type": "control_response", "response": {"subtype": "error", "request_id": "x1", "error": "unsupported"}}));
        assert_eq!(lines[1]["response"]["request_id"], "x2");
    }

    #[test]
    fn pump_keeps_stdin_open_until_the_result() {
        let input = concat!(
            r#"{"type":"system","subtype":"init","session_id":"s","apiKeySource":"none"}"#, "\n",
            r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"a.rs"}}]},"session_id":"s"}"#, "\n",
        );
        let (_, _, fake, _stdin) = run(input);
        assert!(!fake.is_closed());
        let failed = concat!(r#"{"type":"result","is_error":true,"result":"boom","session_id":"s"}"#, "\n");
        let (_, _, fake, _stdin) = run(failed);
        assert!(fake.wait_closed());
    }

    #[test]
    fn pump_kills_on_api_key_init() {
        let input = concat!(
            r#"{"type":"system","subtype":"init","session_id":"s","apiKeySource":"ANTHROPIC_API_KEY"}"#, "\n",
            r#"{"type":"control_request","request_id":"r","request":{"subtype":"can_use_tool","tool_name":"Bash","input":{}}}"#, "\n",
            r#"{"type":"result","is_error":false,"result":"x","session_id":"s"}"#, "\n",
        );
        let (kill, items, fake, _stdin) = run(input);
        assert!(matches!(kill, Some(KillReason::NotSubscription(_))));
        assert!(items.is_empty());
        assert!(fake.wait_closed());
        assert!(fake.lines().is_empty());
    }

    #[test]
    fn pump_kills_on_overage() {
        let input = concat!(
            r#"{"type":"system","subtype":"init","session_id":"s","apiKeySource":"none"}"#, "\n",
            r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed","isUsingOverage":true,"resetsAt":100},"session_id":"s"}"#, "\n",
            r#"{"type":"control_request","request_id":"r","request":{"subtype":"can_use_tool","tool_name":"Bash","input":{}}}"#, "\n",
            r#"{"type":"result","is_error":false,"result":"x","session_id":"s"}"#, "\n",
        );
        let (kill, items, fake, _stdin) = run(input);
        assert_eq!(kill, Some(KillReason::Overage { resets_at: Some(100) }));
        assert_eq!(items.len(), 3);
        assert_eq!(items[2], StreamItem::Overage { resets_at: Some(100) });
        assert!(fake.wait_closed());
    }
}
