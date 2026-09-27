//! Permission prompts Perch can answer (design v1.0 D3–D5).
//!
//! - **Watch**: a PermissionRequest hook from any other Claude Code session. Claude Code shows its own prompt at the
//!   same moment and never waits for Perch. With `watchApprovals` on, the hook's worker holds the HTTP response
//!   (at most `MAX_HOLDS` at once) until the user answers in Perch, the request is resolved elsewhere, the hold
//!   ends, or the feature is turned off.
//! - **Ask**: a `can_use_tool` control request from one of Perch's own Ask runs, answered on that run's stdin.
//!
//! A request Perch isn't holding (watch off, too many holds, the hold ended) stays as a marker until the session
//! resolves it, so "needs input" clears only once nothing of that session is waiting.

use std::{
    sync::{mpsc, Mutex},
    time::Duration,
};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    events::{Kind, PetEvent, Source},
    hook_server::EMPTY_ANSWER,
    locks::lock,
    normalize,
    runner::{self, StdinWriter},
    store,
    threads::{self, ThreadStatus},
};

/// Concurrent watch holds. Beyond this, a request is answered "no decision" at once.
pub const MAX_HOLDS: usize = 16;
/// The longest watch hold; the hook entry's own timeout (300 s) is longer, so Perch always answers first.
pub const MAX_HOLD: Duration = Duration::from_secs(240);
/// An Ask request nobody answers is denied after this long.
pub const ASK_CAP_MS: i64 = 10 * 60_000;
pub const DENY_MESSAGE: &str = "The user declined this in Perch.";
pub const NO_ANSWER_MESSAGE: &str = "No answer in Perch.";
pub const STOPPED_MESSAGE: &str = "Stopped in Perch.";
pub const NEW_CHAT_MESSAGE: &str = "The user started a new chat in Perch.";
pub const QUIT_MESSAGE: &str = "Perch quit before this was answered.";
const GONE: &str = "That request was already answered or is no longer waiting.";
const NO_ALWAYS: &str = "Claude Code didn't offer a rule for this request.";
/// Why a request can only be denied in Perch: part of it can't be shown.
pub const TOO_LONG: &str = "Too long to review here. Answer in Claude Code.";
/// Markers are forgotten this long after their request arrived.
pub const MARKER_KEEP_MS: i64 = 60 * 60_000;
/// Oldest markers make room beyond this many entries.
pub const MAX_ENTRIES: usize = 256;
const SUMMARY_KEYS: [&str; 6] = ["command", "file_path", "notebook_path", "url", "path", "pattern"];
const SUMMARY_JSON_CHARS: usize = 300;
/// The exact command, path or URL is shown whole up to this; beyond it the request can only be denied here.
pub const SUMMARY_MAX_CHARS: usize = 4000;
/// The full input, as pretty JSON, is shown up to this; beyond it the request can only be denied here.
pub const DETAILS_MAX_CHARS: usize = 16 * 1024;
const DESTINATIONS: [&str; 4] = ["session", "localSettings", "projectSettings", "userSettings"];
/// The only modes "Always allow" may switch to, and only for the session.
const SESSION_MODES: [&str; 3] = ["acceptEdits", "default", "plan"];
const SANDBOX_RISK: &str = "Runs outside the sandbox";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Decision {
    Allow,
    Deny,
    Always,
}

/// A request the user can answer in Perch (snapshot `approvals`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingApproval {
    pub id: String,
    pub session_id: String,
    pub project: String,
    pub project_name: String,
    pub source: Source,
    pub tool_name: String,
    /// Claude Code's own name for an MCP tool shown under a display name, e.g. `mcp__github__create_issue`.
    pub raw_tool_name: Option<String>,
    /// The exact command, file path or URL; otherwise the input as compact JSON.
    pub summary: String,
    pub description: Option<String>,
    /// The whole tool input as pretty JSON (at most `DETAILS_MAX_CHARS`).
    pub details: String,
    /// The headline doesn't show everything that's being approved, so the card opens the details.
    pub lossy: bool,
    /// Part of the request can't be shown here, so it can only be denied.
    pub too_long: bool,
    /// Short warnings for risky flags, e.g. "Runs outside the sandbox".
    pub risks: Vec<String>,
    pub can_always_allow: bool,
    pub always_label: Option<String>,
    pub always_detail: Option<String>,
    /// Epoch ms when a watch hold ends; null for Ask requests.
    pub expires_at: Option<i64>,
}

/// One permission request, from a hook body or a `can_use_tool` control request.
#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    pub session_id: String,
    pub project: String,
    pub tool_name: String,
    pub display_name: Option<String>,
    pub input: Value,
    pub description: Option<String>,
    /// `permission_suggestions` as Claude Code sent them.
    pub suggestions: Value,
}

fn text(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).map(str::to_string)
}

impl Request {
    /// A PermissionRequest hook body. None without a session id.
    pub fn from_hook(body: &Value) -> Option<Self> {
        let session_id = body.get("session_id")?.as_str()?.to_string();
        let input = body.get("tool_input").cloned().unwrap_or(Value::Null);
        Some(Self {
            session_id,
            project: body.get("cwd").and_then(Value::as_str).unwrap_or("").to_string(),
            tool_name: text(body, "tool_name").unwrap_or_else(|| "Tool".into()),
            display_name: None,
            description: text(&input, "description"),
            input,
            suggestions: body.get("permission_suggestions").cloned().unwrap_or(Value::Null),
        })
    }

    /// The body of a `can_use_tool` control request from an Ask run.
    pub fn from_can_use_tool(session_id: &str, project: &str, request: &Value) -> Self {
        let input = request.get("input").cloned().unwrap_or(Value::Null);
        Self {
            session_id: session_id.to_string(),
            project: project.to_string(),
            tool_name: text(request, "tool_name").unwrap_or_else(|| "Tool".into()),
            display_name: text(request, "display_name"),
            description: text(request, "description").or_else(|| text(&input, "description")),
            input,
            suggestions: request.get("permission_suggestions").cloned().unwrap_or(Value::Null),
        }
    }
}

fn cap(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max - 1).collect::<String>())
    }
}

/// The card's headline and how it was made.
struct Headline {
    text: String,
    /// It had to be cut.
    truncated: bool,
    /// It is one exact field (command, path, URL), not a JSON preview.
    exact: bool,
}

fn headline(input: &Value) -> Headline {
    let exact = SUMMARY_KEYS.iter().find_map(|k| input.get(*k).and_then(Value::as_str).filter(|s| !s.trim().is_empty()));
    let (full, max, exact) = match (exact, input) {
        (Some(s), _) => (s.to_string(), SUMMARY_MAX_CHARS, true),
        (None, Value::Null) => (String::new(), SUMMARY_JSON_CHARS, false),
        (None, other) => (other.to_string(), SUMMARY_JSON_CHARS, false),
    };
    let truncated = full.chars().count() > max;
    Headline { text: cap(&full, max), truncated, exact }
}

/// What the card shows in monospace: the exact command, file path or URL, else the input as compact JSON.
#[cfg(test)]
fn summary(input: &Value) -> String {
    headline(input).text
}

/// The input keys a built-in tool's card covers: its headline, the description, and fields that are part of the
/// tool's normal shape (shown in the details). None for tools Perch doesn't know.
fn known_keys(tool: &str) -> Option<&'static [&'static str]> {
    Some(match tool {
        "Bash" | "PowerShell" => &["command", "description", "timeout", "run_in_background"],
        "Read" => &["file_path", "offset", "limit", "pages"],
        "Write" => &["file_path", "content"],
        "Edit" => &["file_path", "old_string", "new_string", "replace_all"],
        "MultiEdit" => &["file_path", "edits"],
        "NotebookEdit" => &["notebook_path", "new_source", "cell_id", "cell_type", "edit_mode"],
        "WebFetch" => &["url", "prompt"],
        "WebSearch" => &["query", "allowed_domains", "blocked_domains"],
        "Glob" => &["pattern", "path"],
        "Grep" => &["pattern", "path", "glob", "type", "output_mode", "-i", "-n", "-A", "-B", "-C", "multiline", "head_limit"],
        _ => return None,
    })
}

/// A value that changes nothing when present: null, false or an empty string.
fn trivial(v: &Value) -> bool {
    matches!(v, Value::Null | Value::Bool(false)) || v.as_str() == Some("")
}

/// Whether the headline (with the description) leaves out something being approved: a tool Perch doesn't know,
/// an extra non-trivial key, or a headline that had to be cut.
fn is_lossy(tool: &str, input: &Value, head: &Headline) -> bool {
    if head.truncated {
        return true;
    }
    match (known_keys(tool), input.as_object()) {
        (None, _) => true,
        (Some(keys), Some(obj)) => obj.iter().any(|(k, v)| !keys.contains(&k.as_str()) && !trivial(v)),
        (Some(_), None) => false,
    }
}

fn risks(input: &Value) -> Vec<String> {
    let mut out = Vec::new();
    if input.get("dangerouslyDisableSandbox").and_then(Value::as_bool) == Some(true) {
        out.push(SANDBOX_RISK.to_string());
    }
    out
}

/// A permission dialog that is really a question or a plan to review, never a plain Allow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialog {
    Question,
    Plan,
}

pub fn dialog_kind(tool: &str) -> Option<Dialog> {
    match tool {
        "AskUserQuestion" => Some(Dialog::Question),
        "ExitPlanMode" => Some(Dialog::Plan),
        _ => None,
    }
}

impl Dialog {
    /// The denial an Ask run gets, worded so Claude carries on sensibly.
    pub fn ask_deny_message(self) -> &'static str {
        match self {
            Dialog::Question => "Perch can't show questions yet. Make a reasonable assumption and say what you assumed.",
            Dialog::Plan => "The user will review the plan in Perch's chat.",
        }
    }

    /// The note the mini chat shows.
    pub fn chat_note(self) -> &'static str {
        match self {
            Dialog::Question => "Claude had a question. Perch can't show questions yet, so Claude was asked to assume and say so.",
            Dialog::Plan => "Claude has a plan ready. Review it below, then reply to go ahead.",
        }
    }
}

/// While a session still has a request waiting, its other activity (a parallel subagent's tool call, a streamed
/// reply) updates the label but keeps the thread in "needs input" (design D5).
pub fn keep_needs_input(mut ev: PetEvent, status: Option<ThreadStatus>, waiting: bool) -> PetEvent {
    let live = matches!(ev.kind, Kind::Prompt | Kind::Step | Kind::ReplyDelta);
    if live && waiting && status == Some(ThreadStatus::NeedsInput) {
        ev.kind = Kind::NeedsYou;
    }
    ev
}

/// A tool name for the log: control characters escaped, at most 80 characters.
pub fn log_safe(name: &str) -> String {
    cap(&name.escape_debug().to_string(), 81)
}

fn keep(s: &Value) -> bool {
    let field = |k: &str| s.get(k).and_then(Value::as_str);
    if !field("destination").is_some_and(|d| DESTINATIONS.contains(&d)) {
        return false;
    }
    let non_empty = |k: &str, ok: fn(&Value) -> bool| s.get(k).and_then(Value::as_array).is_some_and(|a| !a.is_empty() && a.iter().all(ok));
    match field("type") {
        Some("addRules") => {
            field("behavior") == Some("allow") && non_empty("rules", |r| r.get("toolName").and_then(Value::as_str).is_some())
        }
        Some("addDirectories") => non_empty("directories", Value::is_string),
        Some("setMode") => field("mode").is_some_and(|m| SESSION_MODES.contains(&m)) && field("destination") == Some("session"),
        _ => false,
    }
}

/// The suggestions "Always allow" may echo back: well-formed allow rules, working folders, and a switch to
/// `acceptEdits`, `default` or `plan` for the session only (so never `bypassPermissions`).
pub fn keep_suggestions(raw: &Value) -> Vec<Value> {
    raw.as_array().into_iter().flatten().filter(|s| keep(s)).cloned().collect()
}

/// "Allow for this session" when every kept suggestion lasts only for the session, else "Always allow".
pub fn always_label(kept: &[Value]) -> Option<&'static str> {
    if kept.is_empty() {
        return None;
    }
    let session_only = kept.iter().all(|s| s.get("destination").and_then(Value::as_str) == Some("session"));
    Some(if session_only { "Allow for this session" } else { "Always allow" })
}

fn rule_text(r: &Value) -> Option<String> {
    let tool = r.get("toolName")?.as_str()?;
    Some(match r.get("ruleContent").and_then(Value::as_str) {
        Some(c) if !c.is_empty() => format!("{tool}({c})"),
        _ => tool.to_string(),
    })
}

fn describe(s: &Value) -> Option<String> {
    let field = |k: &str| s.get(k).and_then(Value::as_str);
    let dest = field("destination")?;
    let place = |session: &'static str, prep: &str| match dest {
        "session" => session.to_string(),
        "localSettings" => format!("{prep} this project's local settings"),
        "projectSettings" => format!("{prep} this project's shared settings"),
        _ => format!("{prep} your user settings"),
    };
    match field("type")? {
        "addRules" => {
            let rules: Vec<String> = s.get("rules")?.as_array()?.iter().filter_map(rule_text).collect();
            let noun = if rules.len() == 1 { "rule" } else { "rules" };
            Some(format!("adds the {noun} {} {}", rules.join(", "), place("for this session", "to")))
        }
        "setMode" => {
            let what = match field("mode")? {
                "acceptEdits" => "lets Claude Code edit files".to_string(),
                "default" => "makes Claude Code ask before acting".to_string(),
                other => format!("switches Claude Code to {other} mode"),
            };
            Some(format!("{what} {}", place("for the rest of this session", "in")))
        }
        "addDirectories" => {
            let dirs: Vec<&str> = s.get("directories")?.as_array()?.iter().filter_map(Value::as_str).collect();
            let noun = if dirs.len() == 1 { "a working folder" } else { "working folders" };
            Some(format!("adds {} as {noun} {}", dirs.join(", "), place("for this session", "in")))
        }
        _ => None,
    }
}

/// One line saying what the always button does, e.g. "Adds the rule Bash(npm test:*) to this project's local
/// settings".
pub fn always_detail(kept: &[Value]) -> Option<String> {
    let joined = kept.iter().filter_map(describe).collect::<Vec<_>>().join("; ");
    let mut chars = joined.chars();
    chars.next().map(|first| first.to_uppercase().collect::<String>() + chars.as_str())
}

/// The PermissionRequest hook's answer.
pub fn hook_answer(decision: Decision, kept: &[Value]) -> String {
    let d = match decision {
        Decision::Allow => json!({ "behavior": "allow" }),
        Decision::Always => json!({ "behavior": "allow", "updatedPermissions": kept }),
        Decision::Deny => json!({ "behavior": "deny", "message": DENY_MESSAGE }),
    };
    json!({ "hookSpecificOutput": { "hookEventName": "PermissionRequest", "decision": d } }).to_string()
}

/// The control_response line that answers a `can_use_tool` request.
pub fn host_answer(request_id: &str, decision: Decision, input: &Value, kept: &[Value]) -> String {
    let input = if input.is_object() { input.clone() } else { json!({}) };
    match decision {
        Decision::Allow => runner::control_response(request_id, json!({ "behavior": "allow", "updatedInput": input })),
        Decision::Always => runner::control_response(
            request_id,
            json!({ "behavior": "allow", "updatedInput": input, "updatedPermissions": kept }),
        ),
        Decision::Deny => host_deny(request_id, DENY_MESSAGE),
    }
}

pub fn host_deny(request_id: &str, message: &str) -> String {
    runner::control_response(request_id, json!({ "behavior": "deny", "message": message }))
}

/// Who is waiting for the answer.
pub enum Responder {
    /// A held hook worker, blocked on the other end.
    Hook(mpsc::Sender<String>),
    /// An Ask run's stdin and the control request's id.
    Ask { stdin: StdinWriter, request_id: String },
}

struct Entry {
    view: PendingApproval,
    /// Claude Code's tool name, for matching (the card may show a display name).
    tool_name: String,
    input: Value,
    suggestions: Vec<Value>,
    created_at: i64,
    /// None for a marker: a request Perch no longer holds, waiting to be resolved elsewhere.
    responder: Option<Responder>,
}

impl Entry {
    fn too_long(&self) -> bool {
        self.view.too_long
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Allowed,
    Denied,
    /// Claude Code withdrew an Ask request.
    Cancelled,
    /// An Ask request denied after `ASK_CAP_MS`.
    NoAnswer,
}

/// A request that left the registry with an answer (or a withdrawal).
#[derive(Debug, Clone, PartialEq)]
pub struct Resolved {
    pub session_id: String,
    pub project: String,
    pub source: Source,
    /// As the card showed it.
    pub tool_name: String,
    pub raw_tool: String,
    pub input: Value,
    pub outcome: Outcome,
}

impl Resolved {
    /// The update that puts the thread back to work, applied once none of its session's requests wait any more.
    pub fn event(&self, at: i64) -> PetEvent {
        let (kind, label) = match self.outcome {
            Outcome::Allowed | Outcome::Cancelled => (Kind::Step, normalize::step_label(&self.raw_tool, &self.input)),
            Outcome::Denied => (Kind::Blocked, format!("Declined: {}", self.tool_name)),
            Outcome::NoAnswer => (Kind::Blocked, format!("No answer in Perch: {}", self.tool_name)),
        };
        PetEvent {
            session_id: self.session_id.clone(),
            project: self.project.clone(),
            source: self.source,
            kind,
            label: Some(label),
            text: None,
            at,
        }
    }
}

impl Entry {
    fn resolved(self, outcome: Outcome) -> Resolved {
        Resolved {
            session_id: self.view.session_id,
            project: self.view.project,
            source: self.view.source,
            tool_name: self.view.tool_name,
            raw_tool: self.tool_name,
            input: self.input,
            outcome,
        }
    }

    fn is_hold(&self) -> bool {
        matches!(self.responder, Some(Responder::Hook(_)))
    }

    fn is_ask_of(&self, project: &str) -> bool {
        self.view.source == Source::Ask && store::same_path(&self.view.project, project)
    }

    /// Answers a held hook "no decision", so Claude Code's own prompt carries on.
    fn release_hook(&mut self) -> bool {
        if let Some(Responder::Hook(tx)) = &self.responder {
            let _ = tx.send(EMPTY_ANSWER.to_string());
            self.responder = None;
            return true;
        }
        false
    }

    fn deny_ask(&self, message: &str) {
        if let Some(Responder::Ask { stdin, request_id }) = &self.responder {
            stdin.send(host_deny(request_id, message));
        }
    }
}

fn new_id() -> String {
    let bytes: [u8; 16] = rand::random();
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Every request Perch knows is waiting, in arrival order.
#[derive(Default)]
pub struct Registry {
    entries: Vec<Entry>,
    /// `watchApprovals`, mirrored here so turning it off and a new request are decided under one lock.
    watch: bool,
}

impl Registry {
    /// Records a request and returns its id. Without a responder it is a marker from the start.
    pub fn add(&mut self, req: Request, source: Source, responder: Option<Responder>, now: i64, expires_at: Option<i64>) -> String {
        if self.entries.len() >= MAX_ENTRIES {
            if let Some(i) = self.entries.iter().position(|e| e.responder.is_none()) {
                self.entries.remove(i);
            }
        }
        let kept = keep_suggestions(&req.suggestions);
        let id = new_id();
        let head = headline(&req.input);
        let pretty = serde_json::to_string_pretty(&req.input).unwrap_or_default();
        let details_cut = pretty.chars().count() > DETAILS_MAX_CHARS;
        let too_long = details_cut || (head.exact && head.truncated);
        let lossy = too_long || is_lossy(&req.tool_name, &req.input, &head);
        let raw_tool_name = match &req.display_name {
            Some(shown) if req.tool_name.starts_with("mcp__") && *shown != req.tool_name => Some(req.tool_name.clone()),
            _ => None,
        };
        let view = PendingApproval {
            id: id.clone(),
            session_id: req.session_id,
            project_name: threads::display_name(&req.project),
            project: req.project,
            source,
            tool_name: req.display_name.unwrap_or_else(|| req.tool_name.clone()),
            raw_tool_name,
            summary: head.text,
            description: req.description,
            details: cap(&pretty, DETAILS_MAX_CHARS),
            lossy,
            too_long,
            risks: risks(&req.input),
            can_always_allow: !kept.is_empty(),
            always_label: always_label(&kept).map(str::to_string),
            always_detail: always_detail(&kept),
            expires_at,
        };
        self.entries.push(Entry { view, tool_name: req.tool_name, input: req.input, suggestions: kept, created_at: now, responder });
        id
    }

    /// Whether watched requests may be held (`watchApprovals`).
    pub fn watching(&self) -> bool {
        self.watch
    }

    /// Turns watching on or off. Off answers every hold "no decision" at once; returns how many.
    pub fn set_watching(&mut self, enabled: bool) -> usize {
        self.watch = enabled;
        if enabled {
            0
        } else {
            self.release_watch()
        }
    }

    /// Watch requests currently held.
    pub fn holds(&self) -> usize {
        self.entries.iter().filter(|e| e.is_hold()).count()
    }

    /// Requests the user can answer, oldest first.
    pub fn pending(&self) -> Vec<PendingApproval> {
        self.entries.iter().filter(|e| e.responder.is_some()).map(|e| e.view.clone()).collect()
    }

    /// Whether any request of this session is still waiting, answerable or not.
    pub fn waiting(&self, session_id: &str) -> bool {
        self.entries.iter().any(|e| e.view.session_id == session_id)
    }

    fn take_where(&mut self, pred: impl Fn(&Entry) -> bool) -> Vec<Entry> {
        let (gone, kept) = std::mem::take(&mut self.entries).into_iter().partition(|e| pred(e));
        self.entries = kept;
        gone
    }

    /// The user's answer from a card. The first answer wins; later ones, unknown ids and requests no longer held
    /// are harmless errors. "Always" needs a kept suggestion, and a request too long to show can only be denied.
    pub fn answer(&mut self, id: &str, decision: Decision) -> Result<Resolved, String> {
        let i = self.entries.iter().position(|e| e.view.id == id && e.responder.is_some()).ok_or(GONE)?;
        if decision != Decision::Deny && self.entries[i].too_long() {
            return Err(TOO_LONG.into());
        }
        if decision == Decision::Always && self.entries[i].suggestions.is_empty() {
            return Err(NO_ALWAYS.into());
        }
        let e = self.entries.remove(i);
        match &e.responder {
            Some(Responder::Hook(tx)) => {
                let _ = tx.send(hook_answer(decision, &e.suggestions));
            }
            Some(Responder::Ask { stdin, request_id }) => {
                stdin.send(host_answer(request_id, decision, &e.input, &e.suggestions));
            }
            None => {}
        }
        let outcome = if decision == Decision::Deny { Outcome::Denied } else { Outcome::Allowed };
        Ok(e.resolved(outcome))
    }

    /// A hold ended: the request stays as a marker. False if it was already answered or released.
    pub fn release(&mut self, id: &str) -> bool {
        match self.entries.iter_mut().find(|e| e.view.id == id) {
            Some(e) if e.is_hold() => {
                e.responder = None;
                true
            }
            _ => false,
        }
    }

    /// Watch requests of `session_id` for `tool` that were resolved in Claude Code: with `input`, only an equal
    /// input matches (there is no tool_use_id). Held hooks are answered "no decision". Returns how many.
    pub fn resolve_elsewhere(&mut self, session_id: &str, tool: &str, input: Option<&Value>) -> usize {
        let gone = self.take_where(|e| {
            e.view.source == Source::Watch && e.view.session_id == session_id && e.tool_name == tool && input.is_none_or(|i| *i == e.input)
        });
        Self::release_all_hooks(gone)
    }

    /// The oldest watch request of `session_id` for `tool`, when a tool result matched none exactly. Returns how many.
    pub fn resolve_oldest(&mut self, session_id: &str, tool: &str) -> usize {
        let Some(i) = self
            .entries
            .iter()
            .position(|e| e.view.source == Source::Watch && e.view.session_id == session_id && e.tool_name == tool)
        else {
            return 0;
        };
        self.entries.remove(i).release_hook();
        1
    }

    /// Every watch request of `session_id` (a new prompt, a stop or the session's end resolved them).
    pub fn clear_session(&mut self, session_id: &str) -> usize {
        let gone = self.take_where(|e| e.view.source == Source::Watch && e.view.session_id == session_id);
        Self::release_all_hooks(gone)
    }

    fn release_all_hooks(gone: Vec<Entry>) -> usize {
        let n = gone.len();
        for mut e in gone {
            e.release_hook();
        }
        n
    }

    /// Watch turned off: every hold is answered "no decision" and stays as a marker. Returns how many.
    pub fn release_watch(&mut self) -> usize {
        self.entries.iter_mut().map(Entry::release_hook).filter(|released| *released).count()
    }

    /// Claude Code withdrew an Ask request (`control_cancel_request`).
    pub fn cancel_ask(&mut self, project: &str, request_id: &str) -> Option<Resolved> {
        let i = self.entries.iter().position(|e| {
            e.is_ask_of(project) && matches!(&e.responder, Some(Responder::Ask { request_id: r, .. }) if r == request_id)
        })?;
        Some(self.entries.remove(i).resolved(Outcome::Cancelled))
    }

    /// Stop, New chat: denies every request of the Ask run in `project` with `message`. Returns how many.
    pub fn deny_ask_run(&mut self, project: &str, message: &str) -> usize {
        let gone = self.take_where(|e| e.is_ask_of(project));
        gone.iter().for_each(|e| e.deny_ask(message));
        gone.len()
    }

    /// The Ask run in `project` ended: its requests can't be answered any more. Returns how many.
    pub fn drop_ask_run(&mut self, project: &str) -> usize {
        self.take_where(|e| e.is_ask_of(project)).len()
    }

    /// Denies Ask requests nobody answered for `ASK_CAP_MS`.
    pub fn deny_overdue_asks(&mut self, now: i64) -> Vec<Resolved> {
        let gone = self.take_where(|e| e.view.source == Source::Ask && now - e.created_at >= ASK_CAP_MS);
        gone.into_iter()
            .map(|e| {
                e.deny_ask(NO_ANSWER_MESSAGE);
                e.resolved(Outcome::NoAnswer)
            })
            .collect()
    }

    /// Forgets markers older than `MARKER_KEEP_MS`. Returns how many.
    pub fn prune(&mut self, now: i64) -> usize {
        self.take_where(|e| e.responder.is_none() && now - e.created_at >= MARKER_KEEP_MS).len()
    }

    /// Perch is quitting: held hooks get "no decision" and Ask requests are denied with `message`.
    pub fn release_all(&mut self, message: &str) -> usize {
        let hooks = self.release_watch();
        let asks = self.take_where(|e| e.view.source == Source::Ask);
        asks.iter().for_each(|e| e.deny_ask(message));
        hooks + asks.len()
    }
}

/// How a watched PermissionRequest is handled (whether watching is on lives in the registry).
#[derive(Debug, Clone, Copy)]
pub struct WatchPolicy {
    /// `approvalHoldSecs` (capped at `MAX_HOLD`).
    pub hold: Duration,
    /// The session is one of Perch's own Ask runs, which are answered on their stdin instead.
    pub own_ask: bool,
}

/// Handles a PermissionRequest hook on its worker and returns the response body (design D3). Blocks while the
/// request is held: until the user answers, the request is resolved elsewhere or released, or the hold ends.
/// `changed` runs whenever the answerable requests change here.
pub fn on_watch_request(reg: &Mutex<Registry>, body: &Value, policy: WatchPolicy, now: i64, changed: &dyn Fn()) -> String {
    if policy.own_ask {
        return EMPTY_ANSWER.to_string();
    }
    let Some(req) = Request::from_hook(body) else { return EMPTY_ANSWER.to_string() };
    let hold = policy.hold.min(MAX_HOLD);
    let (tx, rx) = mpsc::channel();
    let id = {
        let mut r = lock(reg);
        // Questions and plans are left to Claude Code's own dialog: they must never get a generic Allow.
        let dialog = dialog_kind(&req.tool_name).is_some();
        if dialog || !r.watching() || r.holds() >= MAX_HOLDS {
            // Nothing is held, but "needs input" still waits for this request to be resolved.
            r.add(req, Source::Watch, None, now, None);
            return EMPTY_ANSWER.to_string();
        }
        let expires_at = now + i64::try_from(hold.as_millis()).unwrap_or(i64::MAX);
        r.add(req, Source::Watch, Some(Responder::Hook(tx)), now, Some(expires_at))
    };
    changed();
    match rx.recv_timeout(hold) {
        Ok(answer) => answer,
        Err(_) => {
            let released = lock(reg).release(&id);
            if released {
                changed();
            }
            // An answer that raced the end of the hold still counts.
            rx.try_recv().unwrap_or_else(|_| EMPTY_ANSWER.to_string())
        }
    }
}

/// Clears the watch requests a hook event shows were resolved (design D5). Returns how many were removed.
/// - PostToolUse, PostToolUseFailure: the same tool with an equal input ran; if none matches exactly, the
///   session's oldest request for that tool (the input may come back slightly different).
/// - PermissionDenied: the same tool was denied.
/// - UserPromptSubmit, Stop, StopFailure, SessionEnd: all of that session's requests.
pub fn apply_hook_event(reg: &mut Registry, body: &Value) -> usize {
    let Some(sid) = body.get("session_id").and_then(Value::as_str) else { return 0 };
    let tool = body.get("tool_name").and_then(Value::as_str).unwrap_or("");
    match body.get("hook_event_name").and_then(Value::as_str) {
        Some("PostToolUse" | "PostToolUseFailure") => {
            match reg.resolve_elsewhere(sid, tool, Some(body.get("tool_input").unwrap_or(&normalize::NULL))) {
                0 => reg.resolve_oldest(sid, tool),
                n => n,
            }
        }
        Some("PermissionDenied") => reg.resolve_elsewhere(sid, tool, None),
        Some("UserPromptSubmit" | "Stop" | "StopFailure" | "SessionEnd") => reg.clear_session(sid),
        _ => 0,
    }
}

/// After a tool ran or was denied: the update that puts a thread waiting on a prompt back to work, once none of
/// its session's requests are waiting (design D5).
pub fn resume_after(reg: &Registry, status: Option<ThreadStatus>, body: &Value, now: i64) -> Option<PetEvent> {
    let ev = normalize::from_hook_after_tool(body, now)?;
    (status == Some(ThreadStatus::NeedsInput) && !reg.waiting(&ev.session_id)).then_some(ev)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        hook_server::HookServer,
        runner::fake_stdin,
        threads::{ThreadStatus, Threads},
    };
    use serde_json::json;
    use std::{
        sync::{Arc, Mutex},
        time::{Duration, Instant},
    };

    fn body(sid: &str, tool: &str, input: Value) -> Value {
        json!({
            "session_id": sid, "cwd": "C:\\Users\\me\\proj", "hook_event_name": "PermissionRequest",
            "tool_name": tool, "tool_input": input,
            "permission_suggestions": [{"type": "addRules", "rules": [{"toolName": tool, "ruleContent": "npm test:*"}], "behavior": "allow", "destination": "localSettings"}]
        })
    }

    fn req(sid: &str, tool: &str, input: Value) -> Request {
        Request::from_hook(&body(sid, tool, input)).unwrap()
    }

    fn hook_bodies() -> Vec<Value> {
        include_str!("../tests/fixtures/cc2.1.282_hook_bodies.jsonl").lines().map(|l| serde_json::from_str(l).unwrap()).collect()
    }

    fn can_use_tool(fixture: &str) -> Value {
        fixture
            .lines()
            .map(|l| serde_json::from_str::<Value>(l).unwrap())
            .find(|v| v["type"] == "control_request")
            .unwrap()
    }

    // ---- Answers ----

    #[test]
    fn hook_answers_for_allow_deny_and_always() {
        let kept = vec![json!({"type": "setMode", "mode": "acceptEdits", "destination": "session"})];
        let parse = |s: String| serde_json::from_str::<Value>(&s).unwrap();
        assert_eq!(
            parse(hook_answer(Decision::Allow, &kept)),
            json!({"hookSpecificOutput": {"hookEventName": "PermissionRequest", "decision": {"behavior": "allow"}}})
        );
        assert_eq!(
            parse(hook_answer(Decision::Always, &kept)),
            json!({"hookSpecificOutput": {"hookEventName": "PermissionRequest", "decision": {"behavior": "allow", "updatedPermissions": kept}}})
        );
        assert_eq!(
            parse(hook_answer(Decision::Deny, &kept)),
            json!({"hookSpecificOutput": {"hookEventName": "PermissionRequest", "decision": {"behavior": "deny", "message": "The user declined this in Perch."}}})
        );
    }

    #[test]
    fn host_answers_echo_the_input() {
        let input = json!({"command": "echo hi", "description": "Say hi"});
        let kept = vec![json!({"type": "addDirectories", "directories": ["C:\\Users\\me\\proj"], "destination": "session"})];
        let parse = |s: String| serde_json::from_str::<Value>(&s).unwrap();
        assert_eq!(
            parse(host_answer("r1", Decision::Allow, &input, &kept)),
            json!({"type": "control_response", "response": {"subtype": "success", "request_id": "r1", "response": {"behavior": "allow", "updatedInput": input}}})
        );
        assert_eq!(
            parse(host_answer("r1", Decision::Always, &input, &kept)),
            json!({"type": "control_response", "response": {"subtype": "success", "request_id": "r1", "response": {"behavior": "allow", "updatedInput": input, "updatedPermissions": kept}}})
        );
        assert_eq!(
            parse(host_answer("r1", Decision::Deny, &input, &kept)),
            json!({"type": "control_response", "response": {"subtype": "success", "request_id": "r1", "response": {"behavior": "deny", "message": "The user declined this in Perch."}}})
        );
        // A missing or odd input still answers with an object.
        assert_eq!(parse(host_answer("r2", Decision::Allow, &Value::Null, &[]))["response"]["response"]["updatedInput"], json!({}));
    }

    // ---- Suggestions and labels ----

    #[test]
    fn never_offers_bypass_permissions() {
        let raw = json!([
            {"type": "setMode", "mode": "bypassPermissions", "destination": "session"},
            {"type": "setMode", "mode": "BypassPermissions", "destination": "userSettings"},
            {"type": "setMode", "mode": "acceptEdits", "destination": "session"},
        ]);
        assert_eq!(keep_suggestions(&raw), vec![json!({"type": "setMode", "mode": "acceptEdits", "destination": "session"})]);
        let only_bypass = json!([{"type": "setMode", "mode": "bypassPermissions", "destination": "session"}]);
        assert!(keep_suggestions(&only_bypass).is_empty());
    }

    #[test]
    fn modes_are_limited_to_a_short_list_for_the_session() {
        let mode = |m: &str, d: &str| json!({"type": "setMode", "mode": m, "destination": d});
        let raw = json!([
            mode("acceptEdits", "session"),
            mode("default", "session"),
            mode("plan", "session"),
            mode("acceptEdits", "localSettings"),
            mode("acceptEdits", "userSettings"),
            mode("dontAsk", "session"),
            mode("auto", "session"),
            mode("bypassPermissions", "session"),
        ]);
        assert_eq!(keep_suggestions(&raw), vec![mode("acceptEdits", "session"), mode("default", "session"), mode("plan", "session")]);
    }

    #[test]
    fn keeps_only_well_formed_allow_suggestions() {
        let rule = json!({"type": "addRules", "rules": [{"toolName": "Bash", "ruleContent": "npm test:*"}], "behavior": "allow", "destination": "localSettings"});
        let dirs = json!({"type": "addDirectories", "directories": ["C:\\a"], "destination": "session"});
        let raw = json!([
            rule,
            dirs,
            {"type": "addRules", "rules": [{"toolName": "Bash"}], "behavior": "deny", "destination": "localSettings"},
            {"type": "addRules", "rules": [], "behavior": "allow", "destination": "localSettings"},
            {"type": "addRules", "rules": [{"ruleContent": "x"}], "behavior": "allow", "destination": "localSettings"},
            {"type": "removeRules", "rules": [{"toolName": "Bash"}], "behavior": "allow", "destination": "localSettings"},
            {"type": "addDirectories", "directories": [], "destination": "session"},
            {"type": "addDirectories", "directories": ["C:\\a"], "destination": "somewhere"},
            {"type": "setMode", "destination": "session"},
            "setMode",
            7,
        ]);
        assert_eq!(keep_suggestions(&raw), vec![rule, dirs]);
        assert!(keep_suggestions(&Value::Null).is_empty());
        assert!(keep_suggestions(&json!({"type": "setMode"})).is_empty());
    }

    #[test]
    fn always_labels_and_details() {
        let session_mode = json!({"type": "setMode", "mode": "acceptEdits", "destination": "session"});
        let session_dir = json!({"type": "addDirectories", "directories": ["C:\\Users\\me\\proj"], "destination": "session"});
        let local_rule = json!({"type": "addRules", "rules": [{"toolName": "Bash", "ruleContent": "npm test:*"}], "behavior": "allow", "destination": "localSettings"});
        assert_eq!(always_label(&[]), None);
        assert_eq!(always_label(&[session_mode.clone(), session_dir.clone()]), Some("Allow for this session"));
        assert_eq!(always_label(&[session_mode.clone(), local_rule.clone()]), Some("Always allow"));
        assert_eq!(always_label(std::slice::from_ref(&local_rule)), Some("Always allow"));

        assert_eq!(always_detail(&[]), None);
        assert_eq!(always_detail(&[local_rule]).as_deref(), Some("Adds the rule Bash(npm test:*) to this project's local settings"));
        assert_eq!(always_detail(std::slice::from_ref(&session_mode)).as_deref(), Some("Lets Claude Code edit files for the rest of this session"));
        assert_eq!(
            always_detail(std::slice::from_ref(&session_dir)).as_deref(),
            Some("Adds C:\\Users\\me\\proj as a working folder for this session")
        );
        assert_eq!(
            always_detail(&[session_mode, session_dir]).as_deref(),
            Some("Lets Claude Code edit files for the rest of this session; adds C:\\Users\\me\\proj as a working folder for this session")
        );
        let many = json!({"type": "addRules", "rules": [{"toolName": "WebFetch"}, {"toolName": "Bash", "ruleContent": "ls:*"}], "behavior": "allow", "destination": "userSettings"});
        assert_eq!(always_detail(&[many]).as_deref(), Some("Adds the rules WebFetch, Bash(ls:*) to your user settings"));
        let shared = json!({"type": "addRules", "rules": [{"toolName": "Read"}], "behavior": "allow", "destination": "projectSettings"});
        assert_eq!(always_detail(&[shared]).as_deref(), Some("Adds the rule Read to this project's shared settings"));
        let plan = json!({"type": "setMode", "mode": "plan", "destination": "session"});
        assert_eq!(always_detail(&[plan]).as_deref(), Some("Switches Claude Code to plan mode for the rest of this session"));
    }

    // ---- Summaries ----

    #[test]
    fn summaries_show_the_exact_command_file_or_url() {
        assert_eq!(summary(&json!({"command": "npm test -- --watch=false", "description": "Run tests"})), "npm test -- --watch=false");
        assert_eq!(summary(&json!({"command": "Get-ChildItem -Recurse | Select -First 3"})), "Get-ChildItem -Recurse | Select -First 3");
        assert_eq!(summary(&json!({"file_path": "C:\\proj\\src\\app.tsx", "old_string": "a", "new_string": "b"})), "C:\\proj\\src\\app.tsx");
        assert_eq!(summary(&json!({"file_path": "/home/u/out.txt", "content": "hello"})), "/home/u/out.txt");
        assert_eq!(summary(&json!({"file_path": "C:\\proj\\README.md"})), "C:\\proj\\README.md");
        assert_eq!(summary(&json!({"url": "https://example.com/docs", "prompt": "Summarize"})), "https://example.com/docs");
        assert_eq!(summary(&json!({"notebook_path": "C:\\proj\\a.ipynb", "new_source": "x"})), "C:\\proj\\a.ipynb");
        assert_eq!(summary(&json!({"path": "C:\\other", "pattern": "TODO"})), "C:\\other");
        assert_eq!(summary(&json!({"pattern": "**/*.rs"})), "**/*.rs");
        // Multi-line commands stay exact.
        assert_eq!(summary(&json!({"command": "cat <<'EOF'\n\"quoted\"\nEOF"})), "cat <<'EOF'\n\"quoted\"\nEOF");
    }

    #[test]
    fn other_tools_get_compact_json() {
        // An MCP tool.
        assert_eq!(summary(&json!({"owner": "me", "repo": "perch", "title": "Bug"})), r#"{"owner":"me","repo":"perch","title":"Bug"}"#);
        // A blank known key doesn't count.
        assert_eq!(summary(&json!({"command": "  ", "x": 1})), r#"{"command":"  ","x":1}"#);
        assert_eq!(summary(&Value::Null), "");
        assert_eq!(summary(&json!({})), "{}");
        let long = summary(&json!({"body": "x".repeat(1000)}));
        assert_eq!(long.chars().count(), 300);
        assert!(long.ends_with('…'));
        // A huge command is capped too, visibly.
        let huge = summary(&json!({"command": "y".repeat(10_000)}));
        assert_eq!(huge.chars().count(), 4000);
        assert!(huge.ends_with('…'));
    }

    // ---- Requests ----

    #[test]
    fn reads_the_captured_permission_request_body() {
        let r = Request::from_hook(&hook_bodies()[1]).unwrap();
        assert_eq!(r.session_id, "d962f60d-1e49-4915-ad55-87ede94ac0f6");
        assert_eq!((r.project.as_str(), r.tool_name.as_str()), ("C:\\Users\\me\\proj", "Bash"));
        assert_eq!(r.description.as_deref(), Some("Write \"hi\" to out2.txt file"));
        let mut reg = Registry::default();
        reg.add(r, Source::Watch, None, 1, None);
        let v = reg.entries[0].view.clone();
        assert_eq!((v.tool_name.as_str(), v.summary.as_str(), v.project_name.as_str()), ("Bash", "echo hi > out2.txt", "proj"));
        assert!(v.can_always_allow);
        assert_eq!(v.always_label.as_deref(), Some("Allow for this session"));
        assert_eq!(
            v.always_detail.as_deref(),
            Some("Lets Claude Code edit files for the rest of this session; adds C:\\Users\\me\\proj as a working folder for this session")
        );
        // Not a hook body Perch can hold.
        assert!(Request::from_hook(&json!({"hook_event_name": "PermissionRequest"})).is_none());
        assert!(Request::from_hook(&json!({"session_id": 5})).is_none());
        let odd = Request::from_hook(&json!({"session_id": "s", "tool_name": 3, "tool_input": "x", "permission_suggestions": "y"})).unwrap();
        assert_eq!((odd.tool_name.as_str(), odd.project.as_str(), odd.description), ("Tool", "", None));
    }

    #[test]
    fn reads_the_captured_can_use_tool_request() {
        let line = can_use_tool(include_str!("../tests/fixtures/cc2.1.282_host_deny.ndjson"));
        let r = Request::from_can_use_tool("sid", "C:\\Users\\me\\proj", &line["request"]);
        assert_eq!((r.tool_name.as_str(), r.display_name.as_deref()), ("PowerShell", Some("PowerShell")));
        assert_eq!(r.description.as_deref(), Some("Run the specified shell command"));
        assert_eq!(r.input["command"], "echo hi > sdk-out.txt");
        let mut reg = Registry::default();
        reg.add(r, Source::Ask, None, 1, None);
        let v = &reg.entries[0].view;
        assert_eq!((v.summary.as_str(), v.always_label.as_deref()), ("echo hi > sdk-out.txt", Some("Allow for this session")));
        // display_name wins for the card.
        let named = Request::from_can_use_tool("s", "p", &json!({"tool_name": "mcp__gh__create_issue", "display_name": "Create issue", "input": {}}));
        let mut reg = Registry::default();
        reg.add(named, Source::Ask, None, 1, None);
        assert_eq!(reg.entries[0].view.tool_name, "Create issue");
        assert_eq!(reg.entries[0].tool_name, "mcp__gh__create_issue");
    }

    #[test]
    fn pending_approvals_serialize_for_the_frontend() {
        let mut reg = Registry::default();
        let (tx, _rx) = mpsc::channel();
        let id = reg.add(req("s1", "Bash", json!({"command": "npm test"})), Source::Watch, Some(Responder::Hook(tx)), 1, Some(61_000));
        let v = serde_json::to_value(&reg.pending()[0]).unwrap();
        assert_eq!(
            v,
            json!({
                "id": id, "sessionId": "s1", "project": "C:\\Users\\me\\proj", "projectName": "proj", "source": "watch",
                "toolName": "Bash", "rawToolName": null, "summary": "npm test", "description": null,
                "details": "{\n  \"command\": \"npm test\"\n}", "lossy": false, "tooLong": false, "risks": [],
                "canAlwaysAllow": true,
                "alwaysLabel": "Always allow", "alwaysDetail": "Adds the rule Bash(npm test:*) to this project's local settings",
                "expiresAt": 61_000
            })
        );
        // Ids are random and unguessable.
        let other = reg.add(req("s1", "Bash", json!({})), Source::Watch, None, 1, None);
        assert_ne!(id, other);
        assert_eq!(id.len(), 32);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    }

    fn view_of(tool: &str, input: Value) -> PendingApproval {
        let mut reg = Registry::default();
        reg.add(req("s", tool, input), Source::Watch, None, 1, None);
        reg.entries.remove(0).view
    }

    #[test]
    fn details_show_the_whole_input() {
        let w = view_of("Write", json!({"file_path": "C:\\p\\a.txt", "content": "line 1\nline 2"}));
        assert_eq!(w.details, "{\n  \"file_path\": \"C:\\\\p\\\\a.txt\",\n  \"content\": \"line 1\\nline 2\"\n}");
        let e = view_of("Edit", json!({"file_path": "a.rs", "old_string": "let x = 1;", "new_string": "let x = 2;"}));
        assert!(e.details.contains("\"old_string\": \"let x = 1;\"") && e.details.contains("\"new_string\": \"let x = 2;\""), "{}", e.details);
        assert_eq!(view_of("Bash", Value::Null).details, "null");
    }

    #[test]
    fn lossy_when_the_headline_misses_something() {
        let lossy = |tool: &str, input: Value| view_of(tool, input).lossy;
        // The headline plus the description cover these.
        assert!(!lossy("Bash", json!({"command": "ls", "description": "List"})));
        assert!(!lossy("PowerShell", json!({"command": "dir", "timeout": 1000, "run_in_background": false})));
        assert!(!lossy("Write", json!({"file_path": "a", "content": "x"})));
        assert!(!lossy("Edit", json!({"file_path": "a", "old_string": "x", "new_string": "y", "replace_all": true})));
        assert!(!lossy("Read", json!({"file_path": "a", "offset": 1, "limit": 2})));
        assert!(!lossy("WebFetch", json!({"url": "https://example.com", "prompt": "Summarize"})));
        // A built-in tool whose whole input fits the JSON headline.
        assert!(!lossy("WebSearch", json!({"query": "tauri"})));
        // Trivial values of unknown keys don't count.
        assert!(!lossy("Bash", json!({"command": "ls", "dangerouslyDisableSandbox": false, "extra": null})));
        // Extra, non-trivial keys.
        assert!(lossy("Bash", json!({"command": "ls", "dangerouslyDisableSandbox": true})));
        assert!(lossy("Bash", json!({"command": "ls", "cwd": "C:\\other"})));
        // Tools Perch doesn't know.
        assert!(lossy("mcp__fs__write_file", json!({"path": "a", "content": "x"})));
        assert!(lossy("SomethingNew", json!({"a": 1})));
        // A headline that had to be cut.
        assert!(lossy("WebSearch", json!({"query": "q".repeat(400)})));
    }

    #[test]
    fn risky_flags_get_a_badge() {
        assert_eq!(view_of("Bash", json!({"command": "ls", "dangerouslyDisableSandbox": true})).risks, vec!["Runs outside the sandbox"]);
        assert_eq!(view_of("PowerShell", json!({"command": "dir", "dangerouslyDisableSandbox": true})).risks, vec!["Runs outside the sandbox"]);
        assert!(view_of("Bash", json!({"command": "ls", "dangerouslyDisableSandbox": false})).risks.is_empty());
        assert!(view_of("Bash", json!({"command": "ls"})).risks.is_empty());
    }

    #[test]
    fn too_long_inputs_can_only_be_denied() {
        let big = view_of("Write", json!({"file_path": "a", "content": "z".repeat(DETAILS_MAX_CHARS)}));
        assert!(big.too_long && big.lossy);
        assert_eq!(big.details.chars().count(), DETAILS_MAX_CHARS);
        assert!(big.details.ends_with('…'));
        let long_command = view_of("Bash", json!({"command": "y".repeat(SUMMARY_MAX_CHARS + 1)}));
        assert!(long_command.too_long);
        assert!(!view_of("Bash", json!({"command": "y".repeat(SUMMARY_MAX_CHARS)})).too_long);
        // Only Deny goes through, whoever asks.
        for decision in [Decision::Allow, Decision::Always] {
            let mut reg = Registry::default();
            let (tx, rx) = mpsc::channel();
            let id = reg.add(req("s", "Bash", json!({"command": "y".repeat(SUMMARY_MAX_CHARS + 1)})), Source::Watch, Some(Responder::Hook(tx)), 1, Some(1));
            assert_eq!(reg.answer(&id, decision), Err(TOO_LONG.to_string()));
            assert!(rx.try_recv().is_err());
            assert_eq!(reg.answer(&id, Decision::Deny).unwrap().outcome, Outcome::Denied);
        }
    }

    #[test]
    fn mcp_tools_show_their_raw_name_too() {
        let mut reg = Registry::default();
        let named = Request::from_can_use_tool("s", "p", &json!({"tool_name": "mcp__gh__create_issue", "display_name": "Create issue", "input": {}}));
        reg.add(named, Source::Ask, None, 1, None);
        let unnamed = Request::from_can_use_tool("s", "p", &json!({"tool_name": "mcp__gh__close_issue", "input": {}}));
        reg.add(unnamed, Source::Ask, None, 1, None);
        let builtin = Request::from_can_use_tool("s", "p", &json!({"tool_name": "Bash", "display_name": "Shell", "input": {}}));
        reg.add(builtin, Source::Ask, None, 1, None);
        let v: Vec<_> = reg.entries.iter().map(|e| (e.view.tool_name.clone(), e.view.raw_tool_name.clone())).collect();
        assert_eq!(
            v,
            vec![
                ("Create issue".to_string(), Some("mcp__gh__create_issue".to_string())),
                ("mcp__gh__close_issue".to_string(), None),
                ("Shell".to_string(), None),
            ]
        );
    }

    #[test]
    fn questions_and_plans_are_dialogs_not_permissions() {
        assert_eq!(dialog_kind("AskUserQuestion"), Some(Dialog::Question));
        assert_eq!(dialog_kind("ExitPlanMode"), Some(Dialog::Plan));
        assert_eq!(dialog_kind("Bash"), None);
        assert_eq!(dialog_kind("askuserquestion"), None);
        assert_eq!(
            Dialog::Question.ask_deny_message(),
            "Perch can't show questions yet. Make a reasonable assumption and say what you assumed."
        );
        assert_eq!(Dialog::Plan.ask_deny_message(), "The user will review the plan in Perch's chat.");
        assert!(!Dialog::Question.chat_note().is_empty() && !Dialog::Plan.chat_note().is_empty());
    }

    #[test]
    fn watched_questions_and_plans_are_left_to_claude_code() {
        let reg = Mutex::new(Registry::default());
        lock(&reg).set_watching(true);
        let policy = WatchPolicy { hold: Duration::from_secs(20), own_ask: false };
        for tool in ["AskUserQuestion", "ExitPlanMode"] {
            let t0 = Instant::now();
            let sid = format!("s-{tool}");
            assert_eq!(on_watch_request(&reg, &body(&sid, tool, json!({"questions": []})), policy, 1, &|| {}), "{}", "{tool}");
            assert!(t0.elapsed() < Duration::from_secs(1), "{tool}");
            // Needs input stays until the native dialog is answered, but there's nothing to click in Perch.
            assert!(lock(&reg).waiting(&sid), "{tool}");
        }
        assert!(lock(&reg).pending().is_empty());
        assert_eq!(lock(&reg).holds(), 0);
    }

    #[test]
    fn needs_input_stays_while_a_request_waits() {
        let ev = |kind: Kind, label: Option<&str>| PetEvent {
            session_id: "s1".into(),
            project: "p".into(),
            source: Source::Watch,
            kind,
            label: label.map(str::to_string),
            text: None,
            at: 1,
        };
        let needs = Some(ThreadStatus::NeedsInput);
        for kind in [Kind::Prompt, Kind::Step, Kind::ReplyDelta] {
            let kept = keep_needs_input(ev(kind, Some("Reading a.rs")), needs, true);
            assert_eq!((kept.kind, kept.label.as_deref()), (Kind::NeedsYou, Some("Reading a.rs")), "{kind:?}");
            // Nothing waiting, or not waiting on the user: applied as it came.
            assert_eq!(keep_needs_input(ev(kind, None), needs, false).kind, kind, "{kind:?}");
            assert_eq!(keep_needs_input(ev(kind, None), Some(ThreadStatus::Running), true).kind, kind, "{kind:?}");
            assert_eq!(keep_needs_input(ev(kind, None), None, true).kind, kind, "{kind:?}");
        }
        for kind in [Kind::Done, Kind::Failed, Kind::Ended, Kind::Blocked, Kind::Started] {
            assert_eq!(keep_needs_input(ev(kind, None), needs, true).kind, kind, "{kind:?}");
        }
        // Applied to a thread, the label changes and the status stays.
        let mut threads = Threads::default();
        threads.apply(&ev(Kind::NeedsYou, Some("Needs your approval")));
        let alert = threads.apply(&keep_needs_input(ev(Kind::Step, Some("Subagent: Search")), needs, true)).alert;
        let t = threads.get("s1").unwrap();
        assert_eq!((t.status, t.label.as_deref(), alert), (ThreadStatus::NeedsInput, Some("Subagent: Search"), None));
    }

    #[test]
    fn logged_names_are_escaped_and_short() {
        assert_eq!(log_safe("Bash"), "Bash");
        assert_eq!(log_safe("mcp__x\n\u{1b}[2Jfake"), "mcp__x\\n\\u{1b}[2Jfake");
        assert_eq!(log_safe(&"a".repeat(500)).chars().count(), 81);
    }

    // ---- Registry ----

    fn held(reg: &mut Registry, sid: &str, tool: &str, input: Value) -> (String, mpsc::Receiver<String>) {
        let (tx, rx) = mpsc::channel();
        (reg.add(req(sid, tool, input), Source::Watch, Some(Responder::Hook(tx)), 1, Some(1)), rx)
    }

    #[test]
    fn answering_is_idempotent_and_reports_unknown_ids() {
        let mut reg = Registry::default();
        let (id, rx) = held(&mut reg, "s1", "Bash", json!({"command": "ls"}));
        let resolved = reg.answer(&id, Decision::Allow).unwrap();
        assert_eq!(serde_json::from_str::<Value>(&rx.recv().unwrap()).unwrap()["hookSpecificOutput"]["decision"]["behavior"], "allow");
        assert_eq!((resolved.session_id.as_str(), resolved.outcome), ("s1", Outcome::Allowed));
        assert!(reg.pending().is_empty());
        assert!(!reg.waiting("s1"));
        // A second click and an unknown id are harmless errors.
        assert!(reg.answer(&id, Decision::Deny).is_err());
        assert!(reg.answer("nope", Decision::Allow).is_err());
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn always_needs_a_suggestion() {
        let mut reg = Registry::default();
        let (tx, rx) = mpsc::channel();
        let mut plain = req("s1", "Bash", json!({"command": "ls"}));
        plain.suggestions = Value::Null;
        let id = reg.add(plain, Source::Watch, Some(Responder::Hook(tx)), 1, Some(1));
        assert!(!reg.pending()[0].can_always_allow);
        assert_eq!(reg.pending()[0].always_label, None);
        assert!(reg.answer(&id, Decision::Always).is_err());
        assert!(rx.try_recv().is_err());
        // Still pending, so Allow works.
        assert!(reg.answer(&id, Decision::Allow).is_ok());
        assert!(rx.try_recv().is_ok());
    }

    #[test]
    fn a_released_hold_stays_as_a_marker() {
        let mut reg = Registry::default();
        let (id, _rx) = held(&mut reg, "s1", "Bash", json!({"command": "ls"}));
        assert_eq!(reg.holds(), 1);
        assert!(reg.release(&id));
        assert!(!reg.release(&id));
        assert_eq!(reg.holds(), 0);
        assert!(reg.pending().is_empty());
        assert!(reg.waiting("s1"));
        assert!(reg.answer(&id, Decision::Allow).is_err());
    }

    #[test]
    fn resolved_elsewhere_matches_tool_and_equal_input() {
        let mut reg = Registry::default();
        let (_, rx) = held(&mut reg, "s1", "Bash", json!({"command": "npm test", "description": "Run tests"}));
        // Another tool, another input, another session: no match.
        assert_eq!(reg.resolve_elsewhere("s1", "Read", Some(&json!({"command": "npm test", "description": "Run tests"}))), 0);
        assert_eq!(reg.resolve_elsewhere("s1", "Bash", Some(&json!({"command": "npm ci"}))), 0);
        assert_eq!(reg.resolve_elsewhere("s2", "Bash", Some(&json!({"command": "npm test", "description": "Run tests"}))), 0);
        assert!(rx.try_recv().is_err());
        // Equal input in another key order matches, and the held hook gets "no decision".
        assert_eq!(reg.resolve_elsewhere("s1", "Bash", Some(&json!({"description": "Run tests", "command": "npm test"}))), 1);
        assert_eq!(rx.recv().unwrap(), "{}");
        assert!(!reg.waiting("s1"));
    }

    #[test]
    fn permission_denied_matches_the_tool_only() {
        let mut reg = Registry::default();
        reg.add(req("s1", "Bash", json!({"command": "rm -rf x"})), Source::Watch, None, 1, None);
        reg.add(req("s1", "Write", json!({"file_path": "a"})), Source::Watch, None, 1, None);
        assert_eq!(reg.resolve_elsewhere("s1", "Bash", None), 1);
        assert!(reg.waiting("s1"));
        assert_eq!(reg.resolve_elsewhere("s1", "Write", None), 1);
        assert!(!reg.waiting("s1"));
    }

    #[test]
    fn session_events_clear_every_watched_request_of_the_session() {
        let mut reg = Registry::default();
        let (_, rx1) = held(&mut reg, "s1", "Bash", json!({"command": "a"}));
        reg.add(req("s1", "Write", json!({"file_path": "b"})), Source::Watch, None, 1, None);
        let (_, rx2) = held(&mut reg, "s2", "Bash", json!({"command": "c"}));
        let (stdin, _fake, _join) = fake_stdin::writer();
        reg.add(req("s1", "Bash", json!({})), Source::Ask, Some(Responder::Ask { stdin, request_id: "r".into() }), 1, None);
        for name in ["UserPromptSubmit", "Stop", "StopFailure", "SessionEnd"] {
            let mut r = Registry::default();
            r.add(req("s9", "Bash", json!({})), Source::Watch, None, 1, None);
            assert_eq!(apply_hook_event(&mut r, &json!({"hook_event_name": name, "session_id": "s9"})), 1, "{name}");
            assert!(!r.waiting("s9"), "{name}");
        }
        assert_eq!(apply_hook_event(&mut reg, &json!({"hook_event_name": "Stop", "session_id": "s1"})), 2);
        assert_eq!(rx1.recv().unwrap(), "{}");
        assert!(rx2.try_recv().is_err());
        assert!(reg.waiting("s2"));
        // Ask requests are answered on their own stdin, never cleared by hooks.
        assert_eq!(reg.pending().iter().filter(|a| a.source == Source::Ask).count(), 1);
    }

    #[test]
    fn hook_events_resolve_requests_by_d5() {
        let mut reg = Registry::default();
        reg.add(req("s1", "Bash", json!({"command": "npm test"})), Source::Watch, None, 1, None);
        let post = |name: &str, input: Value| json!({"hook_event_name": name, "session_id": "s1", "tool_name": "Bash", "tool_input": input});
        assert_eq!(apply_hook_event(&mut reg, &post("PreToolUse", json!({"command": "npm test"}))), 0);
        let other_tool = json!({"hook_event_name": "PostToolUse", "session_id": "s1", "tool_name": "Read", "tool_input": {"file_path": "a"}});
        assert_eq!(apply_hook_event(&mut reg, &other_tool), 0);
        assert_eq!(apply_hook_event(&mut reg, &post("PostToolUseFailure", json!({"command": "npm test"}))), 1);
        reg.add(req("s1", "Bash", json!({"command": "npm test"})), Source::Watch, None, 1, None);
        assert_eq!(apply_hook_event(&mut reg, &post("PostToolUse", json!({"command": "npm test"}))), 1);
        reg.add(req("s1", "Bash", json!({"command": "x"})), Source::Watch, None, 1, None);
        assert_eq!(apply_hook_event(&mut reg, &post("PermissionDenied", json!({"command": "y"}))), 1);
        assert_eq!(apply_hook_event(&mut reg, &json!({"hook_event_name": "Stop"})), 0);
        assert_eq!(apply_hook_event(&mut reg, &json!([1])), 0);
    }

    /// D5 fallback: Claude Code may report the input a little differently after the tool ran (for example with
    /// defaults filled in), so a PostToolUse that matches nothing exactly releases the session's oldest request
    /// for that tool.
    #[test]
    fn an_unmatched_tool_result_releases_the_oldest_request_for_that_tool() {
        let mut reg = Registry::default();
        let (_, rx_a) = held(&mut reg, "s1", "Bash", json!({"command": "a"}));
        let (_, rx_b) = held(&mut reg, "s1", "Bash", json!({"command": "b"}));
        reg.add(req("s1", "Write", json!({"file_path": "w"})), Source::Watch, None, 1, None);
        reg.add(req("s2", "Bash", json!({"command": "c"})), Source::Watch, None, 1, None);
        let ran = |name: &str, input: Value| json!({"hook_event_name": name, "session_id": "s1", "tool_name": "Bash", "tool_input": input});
        assert_eq!(apply_hook_event(&mut reg, &ran("PostToolUse", json!({"command": "a", "timeout": 120000}))), 1);
        assert_eq!(rx_a.recv().unwrap(), "{}");
        assert!(rx_b.try_recv().is_err());
        // An exact match still wins over the oldest.
        reg.add(req("s1", "Bash", json!({"command": "d"})), Source::Watch, None, 1, None);
        assert_eq!(apply_hook_event(&mut reg, &ran("PostToolUseFailure", json!({"command": "d"}))), 1);
        assert_eq!(reg.pending().len(), 1, "b is still held");
        assert!(rx_b.try_recv().is_err());
        // Nothing for that tool in the session: nothing to release.
        let other = json!({"hook_event_name": "PostToolUse", "session_id": "s1", "tool_name": "Read", "tool_input": {}});
        assert_eq!(apply_hook_event(&mut reg, &other), 0);
        assert!(reg.waiting("s2"));
    }

    #[test]
    fn turning_watch_off_answers_every_hold() {
        let mut reg = Registry::default();
        let (_, rx1) = held(&mut reg, "s1", "Bash", json!({"command": "a"}));
        let (_, rx2) = held(&mut reg, "s2", "Bash", json!({"command": "b"}));
        assert_eq!(reg.release_watch(), 2);
        assert_eq!((rx1.recv().unwrap(), rx2.recv().unwrap()), ("{}".to_string(), "{}".to_string()));
        assert!(reg.pending().is_empty());
        assert!(reg.waiting("s1") && reg.waiting("s2"));
        assert_eq!(reg.release_watch(), 0);
    }

    /// The flag lives in the registry, so turning watching off and a new request can't interleave: the request
    /// either sees the flag off, or is held and then released by the toggle.
    #[test]
    fn the_watch_flag_is_checked_under_the_registry_lock() {
        let reg = Mutex::new(Registry::default());
        assert!(!lock(&reg).watching());
        lock(&reg).set_watching(true);
        let policy = WatchPolicy { hold: Duration::from_secs(20), own_ask: false };
        let held = std::thread::scope(|scope| {
            let h = scope.spawn(|| on_watch_request(&reg, &body("s1", "Bash", json!({"command": "a"})), policy, 1, &|| {}));
            wait_for("the hold", || lock(&reg).holds() == 1);
            assert_eq!(lock(&reg).set_watching(false), 1);
            h.join().unwrap()
        });
        assert_eq!(held, "{}");
        assert!(!lock(&reg).watching());
        let t0 = Instant::now();
        assert_eq!(on_watch_request(&reg, &body("s2", "Bash", json!({})), policy, 1, &|| {}), "{}");
        assert!(t0.elapsed() < Duration::from_secs(1));
        assert_eq!(lock(&reg).holds(), 0);
        assert!(lock(&reg).waiting("s2"));
    }

    #[test]
    fn markers_are_pruned_and_capped() {
        let mut reg = Registry::default();
        reg.add(req("old", "Bash", json!({})), Source::Watch, None, 0, None);
        let (_, _rx) = held(&mut reg, "live", "Bash", json!({}));
        assert_eq!(reg.prune(MARKER_KEEP_MS - 1), 0);
        assert_eq!(reg.prune(MARKER_KEEP_MS), 1);
        assert!(!reg.waiting("old"));
        assert!(reg.waiting("live"));
        for i in 0..MAX_ENTRIES + 10 {
            reg.add(req(&format!("m{i}"), "Bash", json!({})), Source::Watch, None, 5, None);
        }
        assert!(reg.entries.len() <= MAX_ENTRIES);
        assert_eq!(reg.holds(), 1);
    }

    // ---- Ask requests ----

    fn ask(reg: &mut Registry, stdin: &StdinWriter, project: &str, request_id: &str, now: i64) -> String {
        let r = Request::from_can_use_tool("ask-s", project, &json!({"tool_name": "Bash", "input": {"command": "ls"}}));
        reg.add(r, Source::Ask, Some(Responder::Ask { stdin: stdin.clone(), request_id: request_id.into() }), now, None)
    }

    #[test]
    fn ask_answers_go_to_the_runs_stdin() {
        let (stdin, fake, join) = fake_stdin::writer();
        let mut reg = Registry::default();
        let a = ask(&mut reg, &stdin, "C:\\code\\app", "r1", 1);
        let b = ask(&mut reg, &stdin, "C:\\code\\app", "r2", 1);
        assert_eq!(reg.pending()[0].expires_at, None);
        assert_eq!(reg.answer(&a, Decision::Allow).unwrap().outcome, Outcome::Allowed);
        assert_eq!(reg.answer(&b, Decision::Deny).unwrap().outcome, Outcome::Denied);
        stdin.close();
        join.join().unwrap();
        let lines = fake.lines();
        assert_eq!(lines[0]["response"]["request_id"], "r1");
        assert_eq!(lines[0]["response"]["response"], json!({"behavior": "allow", "updatedInput": {"command": "ls"}}));
        assert_eq!(lines[1]["response"]["response"], json!({"behavior": "deny", "message": "The user declined this in Perch."}));
    }

    #[test]
    fn a_cancelled_ask_request_is_dropped_without_an_answer() {
        let (stdin, fake, _join) = fake_stdin::writer();
        let mut reg = Registry::default();
        ask(&mut reg, &stdin, "C:\\code\\app", "r1", 1);
        assert!(reg.cancel_ask("C:\\code\\other", "r1").is_none());
        assert!(reg.cancel_ask("C:\\code\\app", "r9").is_none());
        let gone = reg.cancel_ask("C:\\code\\app", "r1").unwrap();
        assert_eq!(gone.outcome, Outcome::Cancelled);
        assert!(reg.pending().is_empty());
        assert!(fake.lines().is_empty());
    }

    #[test]
    fn stopping_a_run_denies_its_requests() {
        let (stdin, fake, join) = fake_stdin::writer();
        let (other, other_fake, other_join) = fake_stdin::writer();
        let mut reg = Registry::default();
        ask(&mut reg, &stdin, "C:\\code\\app", "r1", 1);
        ask(&mut reg, &other, "C:\\code\\api", "r2", 1);
        assert_eq!(reg.deny_ask_run("C:\\code\\app\\", STOPPED_MESSAGE), 1);
        assert_eq!(reg.pending().len(), 1);
        assert_eq!(reg.drop_ask_run("C:\\code\\api"), 1);
        assert!(reg.pending().is_empty());
        stdin.close();
        other.close();
        join.join().unwrap();
        other_join.join().unwrap();
        assert_eq!(fake.lines()[0]["response"]["response"], json!({"behavior": "deny", "message": "Stopped in Perch."}));
        assert!(other_fake.lines().is_empty());
    }

    #[test]
    fn unanswered_ask_requests_are_denied_after_ten_minutes() {
        let (stdin, fake, join) = fake_stdin::writer();
        let mut reg = Registry::default();
        ask(&mut reg, &stdin, "C:\\code\\app", "r1", 0);
        ask(&mut reg, &stdin, "C:\\code\\app", "r2", 1_000);
        assert!(reg.deny_overdue_asks(ASK_CAP_MS - 1).is_empty());
        let gone = reg.deny_overdue_asks(ASK_CAP_MS);
        assert_eq!(gone.len(), 1);
        assert_eq!(gone[0].outcome, Outcome::NoAnswer);
        assert_eq!(reg.pending().len(), 1);
        stdin.close();
        join.join().unwrap();
        assert_eq!(fake.lines()[0]["response"], json!({"subtype": "success", "request_id": "r1", "response": {"behavior": "deny", "message": "No answer in Perch."}}));
    }

    #[test]
    fn quitting_answers_everything() {
        let (stdin, fake, join) = fake_stdin::writer();
        let mut reg = Registry::default();
        let (_, rx) = held(&mut reg, "s1", "Bash", json!({"command": "a"}));
        ask(&mut reg, &stdin, "C:\\code\\app", "r1", 1);
        assert_eq!(reg.release_all(QUIT_MESSAGE), 2);
        assert_eq!(rx.recv().unwrap(), "{}");
        assert!(reg.pending().is_empty());
        stdin.close();
        join.join().unwrap();
        assert_eq!(fake.lines()[0]["response"]["response"]["message"], "Perch quit before this was answered.");
    }

    #[test]
    fn resolved_requests_put_the_thread_back_to_work() {
        let r = |outcome| Resolved {
            session_id: "s1".into(),
            project: "C:\\p".into(),
            source: Source::Watch,
            tool_name: "Bash".into(),
            raw_tool: "Bash".into(),
            input: json!({"command": "npm test", "description": "Run the tests"}),
            outcome,
        };
        let e = r(Outcome::Allowed).event(5);
        assert_eq!((e.kind, e.label.as_deref(), e.at), (Kind::Step, Some("Run the tests"), 5));
        assert_eq!(r(Outcome::Cancelled).event(5).kind, Kind::Step);
        let d = r(Outcome::Denied).event(5);
        assert_eq!((d.kind, d.label.as_deref()), (Kind::Blocked, Some("Declined: Bash")));
        let n = r(Outcome::NoAnswer).event(5);
        assert_eq!((n.kind, n.label.as_deref()), (Kind::Blocked, Some("No answer in Perch: Bash")));
    }

    #[test]
    fn a_tool_that_ran_resumes_a_waiting_thread_once_nothing_waits() {
        let mut reg = Registry::default();
        let ran = json!({"hook_event_name": "PostToolUse", "session_id": "s1", "cwd": "C:\\p", "tool_name": "Bash", "tool_input": {"command": "npm test"}});
        let needs = Some(ThreadStatus::NeedsInput);
        let e = resume_after(&reg, needs, &ran, 3).unwrap();
        assert_eq!((e.kind, e.label.as_deref(), e.session_id.as_str()), (Kind::Step, Some("Running npm test"), "s1"));
        // Only a waiting thread, and only when no other request of that session is pending.
        assert!(resume_after(&reg, Some(ThreadStatus::Running), &ran, 3).is_none());
        assert!(resume_after(&reg, None, &ran, 3).is_none());
        reg.add(req("s1", "Write", json!({"file_path": "x"})), Source::Watch, None, 1, None);
        assert!(resume_after(&reg, needs, &ran, 3).is_none());
        let denied = json!({"hook_event_name": "PermissionDenied", "session_id": "s2", "tool_name": "Bash"});
        let d = resume_after(&reg, needs, &denied, 3).unwrap();
        assert_eq!((d.kind, d.label.as_deref()), (Kind::Blocked, Some("Blocked: Bash")));
        assert!(resume_after(&reg, needs, &json!({"hook_event_name": "PreToolUse", "session_id": "s3"}), 3).is_none());
    }

    // ---- Watch holds over real HTTP (design D3) ----

    const TOKEN: &str = "t0k3n";

    struct World {
        server: HookServer,
        base: String,
        reg: Arc<Mutex<Registry>>,
        threads: Arc<Mutex<Threads>>,
        applied: mpsc::Receiver<Value>,
    }

    /// A real hook server wired the way Perch wires it: events clear requests and update threads in order,
    /// PermissionRequest bodies are held on their worker.
    fn world(enabled: bool, hold: Duration) -> World {
        let reg = Arc::new(Mutex::new(Registry::default()));
        lock(&reg).set_watching(enabled);
        let threads = Arc::new(Mutex::new(Threads::default()));
        let (tx, applied) = mpsc::channel();
        let (ev_reg, ev_threads) = (reg.clone(), threads.clone());
        let on_event = move |body: Value| {
            apply_hook_event(&mut lock(&ev_reg), &body);
            if let Some(ev) = crate::normalize::from_hook(&body, 1) {
                let waiting = lock(&ev_reg).waiting(&ev.session_id);
                let status = lock(&ev_threads).get(&ev.session_id).map(|t| t.status);
                let ev = keep_needs_input(ev, status, waiting);
                lock(&ev_threads).apply(&ev);
            }
            let status = body["session_id"].as_str().and_then(|s| lock(&ev_threads).get(s).map(|t| t.status));
            let resume = resume_after(&lock(&ev_reg), status, &body, 2);
            if let Some(ev) = resume {
                lock(&ev_threads).apply(&ev);
            }
            let _ = tx.send(body);
        };
        let hold_reg = reg.clone();
        let on_permission = move |body: Value| {
            let policy = WatchPolicy { hold, own_ask: false };
            on_watch_request(&hold_reg, &body, policy, 1_000, &|| {})
        };
        let server = HookServer::start(0, TOKEN.into(), on_event, on_permission).unwrap();
        let base = format!("http://127.0.0.1:{}/hook/{TOKEN}", server.port);
        World { server, base, reg, threads, applied }
    }

    fn post(url: &str, body: &Value) -> String {
        ureq::post(url).timeout(Duration::from_secs(20)).send_string(&body.to_string()).unwrap().into_string().unwrap()
    }

    fn post_in_background(url: &str, body: Value) -> std::thread::JoinHandle<(String, Duration)> {
        let url = url.to_string();
        std::thread::spawn(move || {
            let t0 = Instant::now();
            let answer = post(&url, &body);
            (answer, t0.elapsed())
        })
    }

    fn wait_for(what: &str, mut ok: impl FnMut() -> bool) {
        let t0 = Instant::now();
        while !ok() {
            assert!(t0.elapsed() < Duration::from_secs(10), "timed out waiting for {what}");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn applied_until(w: &World, name: &str) -> Vec<String> {
        let mut seen = vec![];
        loop {
            let v = w.applied.recv_timeout(Duration::from_secs(5)).unwrap_or_else(|_| panic!("{name} was never applied: {seen:?}"));
            let n = v["hook_event_name"].as_str().unwrap_or("").to_string();
            seen.push(n.clone());
            if n == name {
                return seen;
            }
        }
    }

    fn event(name: &str, sid: &str) -> Value {
        json!({"hook_event_name": name, "session_id": sid, "cwd": "C:\\Users\\me\\other", "tool_name": "Read", "tool_input": {"file_path": "a.rs"}})
    }

    fn status(w: &World, sid: &str) -> Option<ThreadStatus> {
        lock(&w.threads).get(sid).map(|t| t.status)
    }

    #[test]
    fn a_held_request_is_allowed_from_perch_while_other_hooks_keep_flowing() {
        let w = world(true, Duration::from_secs(20));
        let held = post_in_background(&w.base, body("s1", "Bash", json!({"command": "npm test"})));
        wait_for("the hold", || lock(&w.reg).holds() == 1);
        // The request shows as needs input at once, in order with the session's other events.
        assert_eq!(applied_until(&w, "PermissionRequest"), vec!["PermissionRequest"]);
        assert_eq!(status(&w, "s1"), Some(ThreadStatus::NeedsInput));
        // Other sessions' hooks are answered and applied while the request is held.
        for name in ["SessionStart", "UserPromptSubmit", "PreToolUse", "PostToolUse"] {
            let t0 = Instant::now();
            assert_eq!(post(&w.base, &event(name, "s2")), "{}");
            assert!(t0.elapsed() < Duration::from_secs(2), "{name} waited {:?}", t0.elapsed());
            applied_until(&w, name);
        }
        assert_eq!(status(&w, "s2"), Some(ThreadStatus::Running));
        assert_eq!(status(&w, "s1"), Some(ThreadStatus::NeedsInput));
        let pending = lock(&w.reg).pending();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].expires_at, Some(21_000));
        lock(&w.reg).answer(&pending[0].id, Decision::Allow).unwrap();
        let (answer, _) = held.join().unwrap();
        assert_eq!(answer, hook_answer(Decision::Allow, &[]));
        assert!(lock(&w.reg).pending().is_empty());
        w.server.stop();
    }

    #[test]
    fn a_parallel_tool_call_keeps_needs_input_while_a_request_waits() {
        let w = world(true, Duration::from_secs(20));
        let held = post_in_background(&w.base, body("s1", "Bash", json!({"command": "npm test"})));
        wait_for("the hold", || lock(&w.reg).holds() == 1);
        applied_until(&w, "PermissionRequest");
        // A subagent in the same session starts another tool.
        post(&w.base, &json!({"hook_event_name": "PreToolUse", "session_id": "s1", "cwd": "C:\\Users\\me\\proj", "tool_name": "Grep", "tool_input": {"pattern": "x"}}));
        applied_until(&w, "PreToolUse");
        let t = lock(&w.threads).get("s1").cloned().unwrap();
        assert_eq!((t.status, t.label.as_deref()), (ThreadStatus::NeedsInput, Some("Searching")));
        // A new prompt clears the request and the thread moves on.
        post(&w.base, &json!({"hook_event_name": "UserPromptSubmit", "session_id": "s1", "cwd": "C:\\Users\\me\\proj"}));
        applied_until(&w, "UserPromptSubmit");
        assert_eq!(status(&w, "s1"), Some(ThreadStatus::Running));
        assert_eq!(held.join().unwrap().0, "{}");
        w.server.stop();
    }

    #[test]
    fn a_held_request_is_denied_from_perch() {
        let w = world(true, Duration::from_secs(20));
        let held = post_in_background(&w.base, body("s1", "Bash", json!({"command": "rm -rf build"})));
        wait_for("the hold", || lock(&w.reg).holds() == 1);
        let id = lock(&w.reg).pending()[0].id.clone();
        lock(&w.reg).answer(&id, Decision::Deny).unwrap();
        let (answer, _) = held.join().unwrap();
        let v: Value = serde_json::from_str(&answer).unwrap();
        assert_eq!(v["hookSpecificOutput"]["decision"], json!({"behavior": "deny", "message": "The user declined this in Perch."}));
        w.server.stop();
    }

    #[test]
    fn an_always_answer_echoes_the_kept_suggestions() {
        let w = world(true, Duration::from_secs(20));
        let held = post_in_background(&w.base, hook_bodies()[1].clone());
        wait_for("the hold", || lock(&w.reg).holds() == 1);
        let id = lock(&w.reg).pending()[0].id.clone();
        lock(&w.reg).answer(&id, Decision::Always).unwrap();
        let v: Value = serde_json::from_str(&held.join().unwrap().0).unwrap();
        assert_eq!(v["hookSpecificOutput"]["decision"]["updatedPermissions"], hook_bodies()[1]["permission_suggestions"]);
        w.server.stop();
    }

    #[test]
    fn a_hold_that_ends_answers_no_decision_and_leaves_needs_input() {
        let w = world(true, Duration::from_millis(400));
        let held = post_in_background(&w.base, body("s1", "Bash", json!({"command": "npm test"})));
        let (answer, took) = held.join().unwrap();
        assert_eq!(answer, "{}");
        assert!(took >= Duration::from_millis(350), "answered after {took:?}");
        // The card falls back to the plain "needs input" thread.
        assert!(lock(&w.reg).pending().is_empty());
        assert!(lock(&w.reg).waiting("s1"));
        applied_until(&w, "PermissionRequest");
        assert_eq!(status(&w, "s1"), Some(ThreadStatus::NeedsInput));
        // Claude Code's own prompt was answered: the tool ran, so the session is back to work.
        post(&w.base, &json!({"hook_event_name": "PostToolUse", "session_id": "s1", "cwd": "C:\\Users\\me\\proj", "tool_name": "Bash", "tool_input": {"command": "npm test"}}));
        applied_until(&w, "PostToolUse");
        assert!(!lock(&w.reg).waiting("s1"));
        assert_eq!(status(&w, "s1"), Some(ThreadStatus::Running));
        w.server.stop();
    }

    #[test]
    fn a_request_resolved_elsewhere_releases_its_hold() {
        let w = world(true, Duration::from_secs(20));
        let held = post_in_background(&w.base, body("s1", "Bash", json!({"command": "npm test"})));
        wait_for("the hold", || lock(&w.reg).holds() == 1);
        post(&w.base, &json!({"hook_event_name": "UserPromptSubmit", "session_id": "s1", "cwd": "C:\\Users\\me\\proj"}));
        let (answer, took) = held.join().unwrap();
        assert_eq!(answer, "{}");
        assert!(took < Duration::from_secs(10));
        assert!(!lock(&w.reg).waiting("s1"));
        w.server.stop();
    }

    #[test]
    fn with_watch_off_requests_are_answered_at_once() {
        let w = world(false, Duration::from_secs(20));
        let t0 = Instant::now();
        assert_eq!(post(&w.base, &body("s1", "Bash", json!({"command": "npm test"}))), "{}");
        assert!(t0.elapsed() < Duration::from_secs(2), "took {:?}", t0.elapsed());
        assert!(lock(&w.reg).pending().is_empty());
        // Still "needs input" until Claude Code's own prompt is answered.
        applied_until(&w, "PermissionRequest");
        assert_eq!(status(&w, "s1"), Some(ThreadStatus::NeedsInput));
        assert!(lock(&w.reg).waiting("s1"));
        w.server.stop();
    }

    #[test]
    fn requests_beyond_the_hold_bound_are_answered_at_once() {
        let w = world(true, Duration::from_secs(30));
        // One at a time: a burst of new connections can make tiny_http queue one behind a busy pool thread.
        let holds: Vec<_> = (0..MAX_HOLDS)
            .map(|i| {
                let h = post_in_background(&w.base, body(&format!("h{i}"), "Bash", json!({"command": i})));
                wait_for("the next hold", || lock(&w.reg).holds() == i + 1);
                h
            })
            .collect();
        for i in 0..3 {
            let t0 = Instant::now();
            assert_eq!(post(&w.base, &body(&format!("x{i}"), "Bash", json!({"command": "x"}))), "{}");
            assert!(t0.elapsed() < Duration::from_secs(2));
        }
        assert_eq!(lock(&w.reg).holds(), MAX_HOLDS);
        assert_eq!(lock(&w.reg).pending().len(), MAX_HOLDS);
        // Other events still flow with every hold taken.
        assert_eq!(post(&w.base, &event("PreToolUse", "s2")), "{}");
        applied_until(&w, "PreToolUse");
        // Turning the feature off answers every hold.
        assert_eq!(lock(&w.reg).release_watch(), MAX_HOLDS);
        for h in holds {
            assert_eq!(h.join().unwrap().0, "{}");
        }
        w.server.stop();
    }

    #[test]
    fn perchs_own_ask_sessions_are_never_held() {
        let reg = Mutex::new(Registry::default());
        lock(&reg).set_watching(true);
        let policy = WatchPolicy { hold: Duration::from_secs(20), own_ask: true };
        let t0 = Instant::now();
        assert_eq!(on_watch_request(&reg, &body("ask", "Bash", json!({})), policy, 1, &|| {}), "{}");
        assert!(t0.elapsed() < Duration::from_secs(1));
        assert!(!lock(&reg).waiting("ask"));
    }
}
