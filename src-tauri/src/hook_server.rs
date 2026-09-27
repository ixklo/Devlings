//! Perch's localhost hook endpoint: `POST /hook/<token>` with a Claude Code hook body.
//!
//! - Every request is read on its own worker thread, so a slow or held request never blocks the others.
//!   At most `MAX_IN_FLIGHT` requests are handled at once; beyond that the server answers 503 at once.
//! - PermissionRequest bodies go to a synchronous handler on their worker; its return value is the response,
//!   and it may block (Perch holds a request there while it waits for an answer).
//! - Every other body is answered `{}` first, then handed to one processing thread in arrival order.
//! - Size never causes an error answer: a body over `MAX_BODY_BYTES` is drained, answered `{}` and ignored.

use std::{
    collections::BTreeMap,
    io::Read,
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc::{self, RecvTimeoutError, Sender},
        Arc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

use serde_json::Value;

pub const MAX_IN_FLIGHT: usize = 32;
/// Large enough for tool bodies carrying whole files or pasted logs.
pub const MAX_BODY_BYTES: usize = 4 * 1024 * 1024;
/// An oversized body is read and thrown away up to this much, so the answer still reaches the client.
const MAX_DRAIN_BYTES: u64 = 64 * 1024 * 1024;
/// How long later events wait behind a request that hasn't finished arriving (a stalled or very slow client)
/// before they are processed without it.
pub const REORDER_WAIT: Duration = Duration::from_secs(2);
pub const EMPTY_ANSWER: &str = "{}";
pub const PERMISSION_EVENT: &str = "PermissionRequest";

type PermissionHandler = dyn Fn(Value) -> String + Send + Sync + 'static;

pub struct HookServer {
    pub port: u16,
    server: Arc<tiny_http::Server>,
    handle: Option<JoinHandle<()>>,
}

/// Compares two byte strings in time that depends only on their lengths.
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let diff = a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y));
    std::hint::black_box(diff) == 0
}

/// Puts numbered items back in order: each request gets a number when it arrives, and its event (or None,
/// when it has none to process) is released only after every earlier number has been.
struct Reorder<T> {
    next: u64,
    pending: BTreeMap<u64, Option<T>>,
}

impl<T> Reorder<T> {
    fn new() -> Self {
        Self { next: 0, pending: BTreeMap::new() }
    }

    /// Records item `seq` and returns every item that is now next in line. A number that was already given up
    /// on (see `skip_gap`) is dropped.
    fn push(&mut self, seq: u64, item: Option<T>) -> Vec<T> {
        if seq < self.next {
            if item.is_some() {
                log::debug!("Dropped a hook event that arrived after its place in line was skipped");
            }
            return Vec::new();
        }
        self.pending.insert(seq, item);
        self.release()
    }

    /// Whether items are held back behind a number that hasn't arrived.
    fn waiting(&self) -> bool {
        !self.pending.is_empty()
    }

    /// Stops waiting for the missing number(s) before the earliest held item, and returns what is now ready.
    fn skip_gap(&mut self) -> Vec<T> {
        if let Some(&first) = self.pending.keys().next() {
            self.next = self.next.max(first);
        }
        self.release()
    }

    fn release(&mut self) -> Vec<T> {
        let mut ready = Vec::new();
        while let Some(item) = self.pending.remove(&self.next) {
            self.next += 1;
            ready.extend(item);
        }
        ready
    }
}

type Queue = Sender<(u64, Option<Value>)>;

/// A request's place in the processing line. Dropping it without an event (a 404, a PermissionRequest, a broken
/// body, a panic) frees the place so later events aren't held up.
struct Ticket {
    seq: u64,
    queue: Option<Queue>,
}

impl Ticket {
    fn send(mut self, event: Value) {
        if let Some(q) = self.queue.take() {
            let _ = q.send((self.seq, Some(event)));
        }
    }
}

impl Drop for Ticket {
    fn drop(&mut self) {
        if let Some(q) = self.queue.take() {
            let _ = q.send((self.seq, None));
        }
    }
}

fn json_response(status: u16, body: String) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    let header = tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..])
        .expect("a static header is valid");
    tiny_http::Response::from_string(body).with_status_code(status).with_header(header)
}

fn reply(req: tiny_http::Request, status: u16, body: String) {
    let _ = req.respond(json_response(status, body));
}

/// Frees one in-flight slot when the worker ends, even if a handler panics.
struct Slot(Arc<AtomicUsize>);

impl Drop for Slot {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Reads and discards what is left of an oversized body, then acknowledges it without processing it.
fn skip_oversized(mut req: tiny_http::Request) {
    let _ = std::io::copy(&mut req.as_reader().take(MAX_DRAIN_BYTES), &mut std::io::sink());
    log::debug!("Ignored a hook body over {MAX_BODY_BYTES} bytes");
    reply(req, 200, EMPTY_ANSWER.to_string());
}

fn handle(mut req: tiny_http::Request, token: &str, ticket: Ticket, on_permission: &PermissionHandler) {
    let authorized = *req.method() == tiny_http::Method::Post
        && req.url().strip_prefix("/hook/").is_some_and(|t| constant_time_eq(t.as_bytes(), token.as_bytes()));
    if !authorized {
        return reply(req, 404, EMPTY_ANSWER.to_string());
    }
    if req.body_length().is_some_and(|n| n > MAX_BODY_BYTES) {
        return skip_oversized(req);
    }
    let mut body = Vec::new();
    if req.as_reader().take(MAX_BODY_BYTES as u64 + 1).read_to_end(&mut body).is_err() {
        // The client went away mid-body; there is nothing to process.
        return reply(req, 200, EMPTY_ANSWER.to_string());
    }
    if body.len() > MAX_BODY_BYTES {
        return skip_oversized(req);
    }
    // Anything that isn't a JSON hook body is acknowledged and ignored; Claude Code never retries.
    let json = body.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&body);
    let Ok(event) = serde_json::from_slice::<Value>(json) else {
        return reply(req, 200, EMPTY_ANSWER.to_string());
    };
    if event.get("hook_event_name").and_then(Value::as_str) == Some(PERMISSION_EVENT) {
        // Free this request's place in line first: the handler may hold it for a long time.
        drop(ticket);
        // A panic is logged by the panic hook; Claude Code still gets a well-formed "no decision".
        let answer = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| on_permission(event))).unwrap_or_default();
        let answer = if answer.trim().is_empty() { EMPTY_ANSWER.to_string() } else { answer };
        return reply(req, 200, answer);
    }
    // Answer first, so Claude Code never waits on Perch's own work.
    reply(req, 200, EMPTY_ANSWER.to_string());
    ticket.send(event);
}

impl HookServer {
    /// Listens on 127.0.0.1:`port` (0 picks a free port).
    /// - `on_event` gets every body except PermissionRequest, after it was answered `{}`, one at a time on a
    ///   single processing thread, in arrival order.
    /// - `on_permission` gets PermissionRequest bodies on their worker thread and returns the JSON response body
    ///   (empty means `{}`). It may block.
    pub fn start<E, P>(port: u16, token: String, on_event: E, on_permission: P) -> Result<Self, String>
    where
        E: FnMut(Value) + Send + 'static,
        P: Fn(Value) -> String + Send + Sync + 'static,
    {
        Self::start_with_limit(port, token, MAX_IN_FLIGHT, on_event, on_permission)
    }

    fn start_with_limit<E, P>(port: u16, token: String, limit: usize, mut on_event: E, on_permission: P) -> Result<Self, String>
    where
        E: FnMut(Value) + Send + 'static,
        P: Fn(Value) -> String + Send + Sync + 'static,
    {
        let server = tiny_http::Server::http(("127.0.0.1", port))
            .map_err(|e| format!("Couldn't listen on port {port}: {e}"))?;
        let port = server.server_addr().to_ip().map(|a| a.port()).unwrap_or(port);
        let server = Arc::new(server);

        // One thread applies events, in the order their requests arrived. A request that stalls mid-body holds
        // later events back for at most REORDER_WAIT. The thread ends once the listener and every worker are gone.
        let (queue, events) = mpsc::channel::<(u64, Option<Value>)>();
        std::thread::Builder::new()
            .name("perch-hook-events".into())
            .spawn(move || {
                let mut order = Reorder::new();
                // The number being waited for, and since when.
                let mut stall: Option<(u64, Instant)> = None;
                loop {
                    let next = match stall {
                        Some((_, since)) => events.recv_timeout(REORDER_WAIT.saturating_sub(since.elapsed())),
                        None => events.recv().map_err(|_| RecvTimeoutError::Disconnected),
                    };
                    let ready = match next {
                        Ok((seq, item)) => order.push(seq, item),
                        Err(RecvTimeoutError::Timeout) => {
                            log::debug!("Stopped waiting for a hook request that didn't finish arriving");
                            order.skip_gap()
                        }
                        Err(RecvTimeoutError::Disconnected) => break,
                    };
                    for event in ready {
                        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| on_event(event))).is_err() {
                            log::error!("A hook event handler panicked; the event was dropped");
                        }
                    }
                    stall = match stall {
                        _ if !order.waiting() => None,
                        Some((n, since)) if n == order.next => Some((n, since)),
                        _ => Some((order.next, Instant::now())),
                    };
                }
            })
            .map_err(|e| format!("Couldn't start the hook event thread: {e}"))?;

        let listener = server.clone();
        let token: Arc<str> = token.into();
        let on_permission: Arc<PermissionHandler> = Arc::new(on_permission);
        let in_flight = Arc::new(AtomicUsize::new(0));
        let handle = std::thread::spawn(move || {
            for (seq, req) in (0u64..).zip(listener.incoming_requests()) {
                let ticket = Ticket { seq, queue: Some(queue.clone()) };
                if in_flight.fetch_add(1, Ordering::SeqCst) >= limit {
                    in_flight.fetch_sub(1, Ordering::SeqCst);
                    reply(req, 503, EMPTY_ANSWER.to_string());
                    continue;
                }
                let slot = Slot(in_flight.clone());
                let (token, on_permission) = (token.clone(), on_permission.clone());
                let spawned = std::thread::Builder::new().name("perch-hook".into()).spawn(move || {
                    let _slot = slot;
                    handle(req, &token, ticket, &*on_permission);
                });
                if let Err(e) = spawned {
                    // The request, its ticket and its slot went down with the closure; the client sees a 500.
                    log::error!("Couldn't start a hook worker: {e}");
                }
            }
        });
        Ok(Self { port, server, handle: Some(handle) })
    }

    pub fn stop(mut self) {
        self.server.unblock();
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::{Mutex, Once},
        time::{Duration, Instant},
    };

    const TOKEN: &str = "abc";

    /// Status, Content-Type and body of a POST.
    fn post(base: &str, path: &str, body: &str) -> (u16, String, String) {
        status_of(ureq::post(&format!("{base}{path}")).timeout(Duration::from_secs(10)).send_string(body))
    }

    fn status_of(r: Result<ureq::Response, ureq::Error>) -> (u16, String, String) {
        let resp = match r {
            Ok(resp) => resp,
            Err(ureq::Error::Status(_, resp)) => resp,
            Err(e) => panic!("{e}"),
        };
        let status = resp.status();
        let ct = resp.header("Content-Type").unwrap_or("").to_string();
        (status, ct, resp.into_string().unwrap())
    }

    fn no_permission(_: Value) -> String {
        panic!("on_permission shouldn't be called")
    }

    fn quiet_panics() {
        static ONCE: Once = Once::new();
        ONCE.call_once(|| {
            let default = std::panic::take_hook();
            std::panic::set_hook(Box::new(move |info| {
                let text = info.payload().downcast_ref::<&str>().copied().unwrap_or("");
                if !text.starts_with("test panic") {
                    default(info);
                }
            }));
        });
    }

    /// A server whose events go to a channel.
    fn channel_server() -> (HookServer, String, mpsc::Receiver<Value>) {
        let (tx, rx) = mpsc::channel();
        let server = HookServer::start(0, TOKEN.into(), move |v| tx.send(v).unwrap(), no_permission).unwrap();
        let base = format!("http://127.0.0.1:{}", server.port);
        (server, base, rx)
    }

    #[test]
    fn accepts_valid_token_and_rejects_others() {
        let (server, base, rx) = channel_server();
        let (status, ct, body) = post(&base, "/hook/abc", r#"{"hook_event_name":"Stop","session_id":"s1"}"#);
        assert_eq!((status, ct.as_str(), body.as_str()), (200, "application/json", "{}"));
        let got = rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(got["session_id"], "s1");

        for path in ["/hook/wrong", "/hook/abcd", "/hook/ab", "/other/abc"] {
            let (status, ct, _) = post(&base, path, "{}");
            assert_eq!((status, ct.as_str()), (404, "application/json"), "{path}");
        }
        let (status, _, _) = status_of(ureq::get(&format!("{base}/hook/abc")).call());
        assert_eq!(status, 404);

        let (status, _, body) = post(&base, "/hook/abc", "nope");
        assert_eq!((status, body.as_str()), (200, "{}"));
        assert!(rx.recv_timeout(Duration::from_millis(200)).is_err());
        server.stop();
    }

    #[test]
    fn only_permission_requests_get_the_handlers_answer() {
        let (tx, rx) = mpsc::channel();
        let server = HookServer::start(
            0,
            TOKEN.into(),
            move |v| tx.send(v).unwrap(),
            |v| format!(r#"{{"tool":{}}}"#, v["tool_name"]),
        )
        .unwrap();
        let base = format!("http://127.0.0.1:{}", server.port);
        let (_, ct, body) = post(&base, "/hook/abc", r#"{"hook_event_name":"PermissionRequest","tool_name":"Bash"}"#);
        assert_eq!((ct.as_str(), body.as_str()), ("application/json", r#"{"tool":"Bash"}"#));
        // Permission requests don't reach the event thread.
        assert!(rx.recv_timeout(Duration::from_millis(200)).is_err());
        let (_, _, body) = post(&base, "/hook/abc", r#"{"hook_event_name":"PreToolUse","tool_name":"Bash"}"#);
        assert_eq!(body, "{}");
        assert_eq!(rx.recv_timeout(Duration::from_secs(2)).unwrap()["hook_event_name"], "PreToolUse");
        server.stop();
    }

    #[test]
    fn an_empty_permission_answer_is_an_empty_object() {
        let server = HookServer::start(0, TOKEN.into(), |_| {}, |_| String::new()).unwrap();
        let base = format!("http://127.0.0.1:{}", server.port);
        assert_eq!(post(&base, "/hook/abc", r#"{"hook_event_name":"PermissionRequest"}"#).2, "{}");
        server.stop();
    }

    #[test]
    fn events_are_answered_before_they_are_processed() {
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let (done_tx, done_rx) = mpsc::channel();
        let server = HookServer::start(
            0,
            TOKEN.into(),
            move |v| {
                let _ = release_rx.recv_timeout(Duration::from_secs(10));
                done_tx.send(v["n"].as_i64().unwrap()).unwrap();
            },
            no_permission,
        )
        .unwrap();
        let base = format!("http://127.0.0.1:{}", server.port);
        let t0 = Instant::now();
        for n in 0..3 {
            assert_eq!(post(&base, "/hook/abc", &format!(r#"{{"hook_event_name":"PostToolUse","n":{n}}}"#)).0, 200);
        }
        assert!(t0.elapsed() < Duration::from_secs(3), "answers waited for processing: {:?}", t0.elapsed());
        assert!(done_rx.try_recv().is_err());
        for _ in 0..3 {
            release_tx.send(()).unwrap();
        }
        let done: Vec<i64> = (0..3).map(|_| done_rx.recv_timeout(Duration::from_secs(5)).unwrap()).collect();
        assert_eq!(done, vec![0, 1, 2]);
        server.stop();
    }

    #[test]
    fn events_are_processed_in_arrival_order() {
        let (server, base, rx) = channel_server();
        for n in 0..40 {
            post(&base, "/hook/abc", &format!(r#"{{"hook_event_name":"PreToolUse","n":{n}}}"#));
        }
        let got: Vec<i64> = (0..40).map(|_| rx.recv_timeout(Duration::from_secs(5)).unwrap()["n"].as_i64().unwrap()).collect();
        assert_eq!(got, (0..40).collect::<Vec<_>>());
        server.stop();
    }

    #[test]
    fn reorder_releases_items_in_sequence() {
        let mut r = Reorder::new();
        assert!(r.push(1, Some("b")).is_empty());
        assert!(r.push(3, Some("d")).is_empty());
        assert_eq!(r.push(0, Some("a")), vec!["a", "b"]);
        // A number with nothing to process still unblocks the ones after it.
        assert_eq!(r.push(2, None), vec!["d"]);
        assert_eq!(r.push(4, Some("e")), vec!["e"]);
        assert!(r.pending.is_empty());
    }

    #[test]
    fn reorder_can_give_up_on_a_gap() {
        let mut r = Reorder::new();
        assert!(!r.waiting());
        assert!(r.push(1, Some("b")).is_empty());
        assert!(r.push(2, Some("c")).is_empty());
        assert!(r.waiting());
        assert_eq!(r.skip_gap(), vec!["b", "c"]);
        assert!(!r.waiting());
        // The skipped number arriving late is dropped, not buffered forever.
        assert!(r.push(0, Some("a")).is_empty());
        assert!(r.pending.is_empty());
        assert_eq!(r.push(3, Some("d")), vec!["d"]);
        // Nothing to skip: a no-op.
        assert!(r.skip_gap().is_empty());
    }

    #[test]
    fn a_stalled_request_does_not_hold_up_later_events() {
        use std::io::Write;
        let (server, base, rx) = channel_server();
        // Headers promise a body that never finishes arriving, so this request's worker waits on it. (Bodies under
        // a few KB are read by tiny_http before the request is handed over, so only a large one can stall here.)
        let mut stalled = std::net::TcpStream::connect(("127.0.0.1", server.port)).unwrap();
        stalled
            .write_all(b"POST /hook/abc HTTP/1.1\r\nHost: x\r\nContent-Type: application/json\r\nContent-Length: 100000\r\n\r\n{\"hook")
            .unwrap();
        std::thread::sleep(Duration::from_millis(200));
        let t0 = Instant::now();
        assert_eq!(post(&base, "/hook/abc", r#"{"hook_event_name":"Stop","n":1}"#).0, 200);
        let got = rx.recv_timeout(REORDER_WAIT + Duration::from_secs(3)).expect("a later event was held up for good");
        assert_eq!(got["n"], 1);
        assert!(t0.elapsed() >= REORDER_WAIT - Duration::from_millis(100), "released before the wait: {:?}", t0.elapsed());
        drop(stalled);
        server.stop();
    }

    #[test]
    fn a_held_permission_request_does_not_block_events() {
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let release_rx = Mutex::new(release_rx);
        let (tx, rx) = mpsc::channel();
        let server = HookServer::start(
            0,
            TOKEN.into(),
            move |v| tx.send(v).unwrap(),
            move |_| {
                let _ = release_rx.lock().unwrap().recv_timeout(Duration::from_secs(10));
                r#"{"held":true}"#.to_string()
            },
        )
        .unwrap();
        let base = format!("http://127.0.0.1:{}", server.port);
        let held_base = base.clone();
        let held = std::thread::spawn(move || post(&held_base, "/hook/abc", r#"{"hook_event_name":"PermissionRequest"}"#).2);
        std::thread::sleep(Duration::from_millis(150));
        let t0 = Instant::now();
        assert_eq!(post(&base, "/hook/abc", r#"{"hook_event_name":"PreToolUse"}"#).0, 200);
        assert!(t0.elapsed() < Duration::from_secs(2), "blocked for {:?}", t0.elapsed());
        // Its event is processed even though an earlier request is still held.
        assert_eq!(rx.recv_timeout(Duration::from_secs(2)).unwrap()["hook_event_name"], "PreToolUse");
        release_tx.send(()).unwrap();
        assert_eq!(held.join().unwrap(), r#"{"held":true}"#);
        server.stop();
    }

    #[test]
    fn answers_503_when_too_many_are_in_flight() {
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let release_rx = Mutex::new(release_rx);
        let server = HookServer::start_with_limit(0, TOKEN.into(), 1, |_| {}, move |_| {
            let _ = release_rx.lock().unwrap().recv_timeout(Duration::from_secs(10));
            EMPTY_ANSWER.to_string()
        })
        .unwrap();
        let base = format!("http://127.0.0.1:{}", server.port);
        let held_base = base.clone();
        let held = std::thread::spawn(move || post(&held_base, "/hook/abc", r#"{"hook_event_name":"PermissionRequest"}"#).0);
        std::thread::sleep(Duration::from_millis(150));
        let t0 = Instant::now();
        let (status, ct, _) = post(&base, "/hook/abc", "{}");
        assert_eq!((status, ct.as_str()), (503, "application/json"));
        assert!(t0.elapsed() < Duration::from_secs(2));
        release_tx.send(()).unwrap();
        assert_eq!(held.join().unwrap(), 200);
        // Capacity comes back once the held request finishes. With the sender gone, nothing holds any more.
        drop(release_tx);
        assert_eq!(post(&base, "/hook/abc", "{}").0, 200);
        server.stop();
    }

    #[test]
    fn oversized_bodies_are_acknowledged_and_ignored() {
        let (server, base, rx) = channel_server();
        let big = format!(r#"{{"hook_event_name":"PostToolUse","pad":"{}"}}"#, "x".repeat(MAX_BODY_BYTES));
        let (status, ct, body) = post(&base, "/hook/abc", &big);
        assert_eq!((status, ct.as_str(), body.as_str()), (200, "application/json", "{}"));
        assert!(rx.recv_timeout(Duration::from_millis(300)).is_err());
        let fits = format!(r#"{{"hook_event_name":"PostToolUse","pad":"{}"}}"#, "x".repeat(MAX_BODY_BYTES - 100));
        assert_eq!(post(&base, "/hook/abc", &fits).0, 200);
        assert_eq!(rx.recv_timeout(Duration::from_secs(5)).unwrap()["hook_event_name"], "PostToolUse");
        server.stop();
    }

    #[test]
    fn panics_in_handlers_are_contained() {
        quiet_panics();
        let (tx, rx) = mpsc::channel();
        let server = HookServer::start(
            0,
            TOKEN.into(),
            move |v| {
                if v["boom"] == true {
                    panic!("test panic in on_event");
                }
                tx.send(v).unwrap();
            },
            |_| panic!("test panic in on_permission"),
        )
        .unwrap();
        let base = format!("http://127.0.0.1:{}", server.port);
        let (status, ct, body) = post(&base, "/hook/abc", r#"{"hook_event_name":"PermissionRequest"}"#);
        assert_eq!((status, ct.as_str(), body.as_str()), (200, "application/json", "{}"));
        assert_eq!(post(&base, "/hook/abc", r#"{"hook_event_name":"Stop","boom":true}"#).0, 200);
        // The processing thread survives and handles the next event.
        post(&base, "/hook/abc", r#"{"hook_event_name":"Stop","n":2}"#);
        assert_eq!(rx.recv_timeout(Duration::from_secs(2)).unwrap()["n"], 2);
        server.stop();
    }

    #[test]
    fn busy_port_is_an_error() {
        let a = HookServer::start(0, "t".into(), |_| {}, |_| String::new()).unwrap();
        let err = HookServer::start(a.port, "t".into(), |_| {}, |_| String::new()).err().unwrap();
        assert!(err.contains(&a.port.to_string()), "{err}");
        a.stop();
    }

    #[test]
    fn constant_time_comparison() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"ab"));
        assert!(!constant_time_eq(b"", b"a"));
        assert!(constant_time_eq(b"", b""));
    }
}
