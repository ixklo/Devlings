//! Perch's localhost hook endpoint: `POST /hook/<token>` with a Claude Code hook body.
//!
//! Every request is handled on its own worker thread, so a slow or held request never blocks the others.
//! At most `MAX_IN_FLIGHT` requests are handled at once; beyond that the server answers 503 at once.

use std::{
    io::Read,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    thread::JoinHandle,
};

use serde_json::Value;

pub const MAX_IN_FLIGHT: usize = 32;
pub const MAX_BODY_BYTES: usize = 256 * 1024;
pub const EMPTY_ANSWER: &str = "{}";

type Callback = dyn Fn(Value) -> String + Send + Sync + 'static;

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

fn json_response(status: u16, body: String) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    let header = tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..])
        .expect("a static header is valid");
    tiny_http::Response::from_string(body).with_status_code(status).with_header(header)
}

fn reply(req: tiny_http::Request, status: u16, body: String) {
    let _ = req.respond(json_response(status, body));
}

/// Frees one in-flight slot when the worker ends, even if the callback panics.
struct Slot(Arc<AtomicUsize>);

impl Drop for Slot {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

fn handle(mut req: tiny_http::Request, token: &str, on_body: &Callback) {
    let authorized = *req.method() == tiny_http::Method::Post
        && req.url().strip_prefix("/hook/").is_some_and(|t| constant_time_eq(t.as_bytes(), token.as_bytes()));
    if !authorized {
        return reply(req, 404, EMPTY_ANSWER.to_string());
    }
    if req.body_length().is_some_and(|n| n > MAX_BODY_BYTES) {
        return reply(req, 413, EMPTY_ANSWER.to_string());
    }
    let mut body = Vec::new();
    if req.as_reader().take(MAX_BODY_BYTES as u64 + 1).read_to_end(&mut body).is_err() {
        return reply(req, 400, EMPTY_ANSWER.to_string());
    }
    if body.len() > MAX_BODY_BYTES {
        return reply(req, 413, EMPTY_ANSWER.to_string());
    }
    // Anything that isn't a JSON hook body is acknowledged and ignored; Claude Code never retries.
    let json = body.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&body);
    let answer = match serde_json::from_slice::<Value>(json) {
        // A panic is logged by the panic hook; Claude Code still gets a well-formed "no decision".
        Ok(v) => std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| on_body(v))).unwrap_or_default(),
        Err(_) => String::new(),
    };
    let answer = if answer.trim().is_empty() { EMPTY_ANSWER.to_string() } else { answer };
    reply(req, 200, answer);
}

impl HookServer {
    /// Listens on 127.0.0.1:`port` (0 picks a free port). `on_body` gets each parsed hook body on a worker
    /// thread and returns the JSON response body (normally `"{}"`). It must stay fast for normal events.
    pub fn start<F>(port: u16, token: String, on_body: F) -> Result<Self, String>
    where
        F: Fn(Value) -> String + Send + Sync + 'static,
    {
        Self::start_with_limit(port, token, MAX_IN_FLIGHT, on_body)
    }

    fn start_with_limit<F>(port: u16, token: String, limit: usize, on_body: F) -> Result<Self, String>
    where
        F: Fn(Value) -> String + Send + Sync + 'static,
    {
        let server = tiny_http::Server::http(("127.0.0.1", port))
            .map_err(|e| format!("Couldn't listen on port {port}: {e}"))?;
        let port = server.server_addr().to_ip().map(|a| a.port()).unwrap_or(port);
        let server = Arc::new(server);
        let listener = server.clone();
        let token: Arc<str> = token.into();
        let on_body: Arc<Callback> = Arc::new(on_body);
        let in_flight = Arc::new(AtomicUsize::new(0));
        let handle = std::thread::spawn(move || {
            for req in listener.incoming_requests() {
                if in_flight.fetch_add(1, Ordering::SeqCst) >= limit {
                    in_flight.fetch_sub(1, Ordering::SeqCst);
                    reply(req, 503, EMPTY_ANSWER.to_string());
                    continue;
                }
                let slot = Slot(in_flight.clone());
                let (token, on_body) = (token.clone(), on_body.clone());
                let spawned = std::thread::Builder::new().name("perch-hook".into()).spawn(move || {
                    let _slot = slot;
                    handle(req, &token, &*on_body);
                });
                if let Err(e) = spawned {
                    // The request (and its slot) went down with the closure; the client sees a closed connection.
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
        sync::mpsc,
        time::{Duration, Instant},
    };

    const TOKEN: &str = "abc";

    /// Status, Content-Type and body of a POST.
    fn post(base: &str, path: &str, body: &str) -> (u16, String, String) {
        status_of(ureq::post(&format!("{base}{path}")).timeout(Duration::from_secs(5)).send_string(body))
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

    #[test]
    fn accepts_valid_token_and_rejects_others() {
        let (tx, rx) = mpsc::channel();
        let tx = std::sync::Mutex::new(tx);
        let server = HookServer::start(0, TOKEN.into(), move |v| {
            tx.lock().unwrap().send(v).unwrap();
            EMPTY_ANSWER.to_string()
        })
        .unwrap();
        let base = format!("http://127.0.0.1:{}", server.port);

        let (status, ct, body) = post(&base, "/hook/abc", r#"{"hook_event_name":"Stop","session_id":"s1"}"#);
        assert_eq!((status, ct.as_str(), body.as_str()), (200, "application/json", "{}"));
        let got = rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(got["session_id"], "s1");

        let (status, ct, _) = post(&base, "/hook/wrong", "{}");
        assert_eq!((status, ct.as_str()), (404, "application/json"));
        let (status, _, _) = post(&base, "/hook/abcd", "{}");
        assert_eq!(status, 404);
        let (status, _, _) = post(&base, "/hook/ab", "{}");
        assert_eq!(status, 404);
        let (status, _, _) = post(&base, "/other/abc", "{}");
        assert_eq!(status, 404);
        let (status, _, _) = status_of(ureq::get(&format!("{base}/hook/abc")).call());
        assert_eq!(status, 404);

        let (status, _, body) = post(&base, "/hook/abc", "nope");
        assert_eq!((status, body.as_str()), (200, "{}"));
        assert!(rx.recv_timeout(Duration::from_millis(200)).is_err());

        server.stop();
    }

    #[test]
    fn returns_the_callbacks_answer() {
        let server = HookServer::start(0, TOKEN.into(), |v| {
            if v["hook_event_name"] == "PermissionRequest" {
                r#"{"answer":1}"#.to_string()
            } else {
                String::new()
            }
        })
        .unwrap();
        let base = format!("http://127.0.0.1:{}", server.port);
        let (_, ct, body) = post(&base, "/hook/abc", r#"{"hook_event_name":"PermissionRequest"}"#);
        assert_eq!((ct.as_str(), body.as_str()), ("application/json", r#"{"answer":1}"#));
        // An empty answer still goes out as a JSON object.
        let (_, _, body) = post(&base, "/hook/abc", r#"{"hook_event_name":"Stop"}"#);
        assert_eq!(body, "{}");
        server.stop();
    }

    #[test]
    fn a_held_request_does_not_block_others() {
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let release_rx = std::sync::Mutex::new(release_rx);
        let server = HookServer::start(0, TOKEN.into(), move |v| {
            if v["hold"] == true {
                let _ = release_rx.lock().unwrap().recv_timeout(Duration::from_secs(10));
            }
            EMPTY_ANSWER.to_string()
        })
        .unwrap();
        let base = format!("http://127.0.0.1:{}", server.port);
        let held_base = base.clone();
        let held = std::thread::spawn(move || post(&held_base, "/hook/abc", r#"{"hold":true}"#).0);
        std::thread::sleep(Duration::from_millis(150));
        let t0 = Instant::now();
        let (status, _, _) = post(&base, "/hook/abc", r#"{"hook_event_name":"PreToolUse"}"#);
        assert_eq!(status, 200);
        assert!(t0.elapsed() < Duration::from_secs(2), "blocked for {:?}", t0.elapsed());
        release_tx.send(()).unwrap();
        assert_eq!(held.join().unwrap(), 200);
        server.stop();
    }

    #[test]
    fn answers_503_when_too_many_are_in_flight() {
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let release_rx = std::sync::Mutex::new(release_rx);
        let server = HookServer::start_with_limit(0, TOKEN.into(), 1, move |_| {
            let _ = release_rx.lock().unwrap().recv_timeout(Duration::from_secs(10));
            EMPTY_ANSWER.to_string()
        })
        .unwrap();
        let base = format!("http://127.0.0.1:{}", server.port);
        let held_base = base.clone();
        let held = std::thread::spawn(move || post(&held_base, "/hook/abc", "{}").0);
        std::thread::sleep(Duration::from_millis(150));
        let t0 = Instant::now();
        let (status, ct, _) = post(&base, "/hook/abc", "{}");
        assert_eq!((status, ct.as_str()), (503, "application/json"));
        assert!(t0.elapsed() < Duration::from_secs(2));
        release_tx.send(()).unwrap();
        assert_eq!(held.join().unwrap(), 200);
        // Capacity comes back once the held request finishes. With the sender gone, nothing holds any more.
        drop(release_tx);
        let (status, _, _) = post(&base, "/hook/abc", "{}");
        assert_eq!(status, 200);
        server.stop();
    }

    #[test]
    fn rejects_bodies_over_the_cap() {
        let (tx, rx) = mpsc::channel();
        let tx = std::sync::Mutex::new(tx);
        let server = HookServer::start(0, TOKEN.into(), move |v| {
            tx.lock().unwrap().send(v).unwrap();
            EMPTY_ANSWER.to_string()
        })
        .unwrap();
        let base = format!("http://127.0.0.1:{}", server.port);
        let big = format!(r#"{{"pad":"{}"}}"#, "x".repeat(MAX_BODY_BYTES));
        let (status, ct, _) = post(&base, "/hook/abc", &big);
        assert_eq!((status, ct.as_str()), (413, "application/json"));
        assert!(rx.recv_timeout(Duration::from_millis(200)).is_err());
        let fits = format!(r#"{{"pad":"{}"}}"#, "x".repeat(MAX_BODY_BYTES - 20));
        let (status, _, _) = post(&base, "/hook/abc", &fits);
        assert_eq!(status, 200);
        assert!(rx.recv_timeout(Duration::from_secs(2)).is_ok());
        server.stop();
    }

    #[test]
    fn a_panicking_callback_still_answers_json() {
        let server = HookServer::start(0, TOKEN.into(), |v| {
            if v["boom"] == true {
                panic!("callback failed");
            }
            EMPTY_ANSWER.to_string()
        })
        .unwrap();
        let base = format!("http://127.0.0.1:{}", server.port);
        let (status, ct, body) = post(&base, "/hook/abc", r#"{"boom":true}"#);
        assert_eq!((status, ct.as_str(), body.as_str()), (200, "application/json", "{}"));
        assert_eq!(post(&base, "/hook/abc", "{}").0, 200);
        server.stop();
    }

    #[test]
    fn busy_port_is_an_error() {
        let a = HookServer::start(0, "t".into(), |_| EMPTY_ANSWER.to_string()).unwrap();
        let err = HookServer::start(a.port, "t".into(), |_| EMPTY_ANSWER.to_string()).err().unwrap();
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
