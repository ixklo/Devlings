use std::{io::Read, sync::Arc, thread::JoinHandle};

use serde_json::Value;

pub struct HookServer {
    pub port: u16,
    server: Arc<tiny_http::Server>,
    handle: Option<JoinHandle<()>>,
}

impl HookServer {
    pub fn start<F>(port: u16, token: String, on_body: F) -> Result<Self, String>
    where
        F: Fn(Value) + Send + 'static,
    {
        let server = tiny_http::Server::http(("127.0.0.1", port))
            .map_err(|e| format!("Couldn't listen on port {port}: {e}"))?;
        let port = server.server_addr().to_ip().map(|a| a.port()).unwrap_or(port);
        let server = Arc::new(server);
        let listener = server.clone();
        let path = format!("/hook/{token}");
        let handle = std::thread::spawn(move || {
            for mut req in listener.incoming_requests() {
                if *req.method() != tiny_http::Method::Post || req.url() != path {
                    let _ = req.respond(tiny_http::Response::empty(404));
                    continue;
                }
                let mut body = String::new();
                let read_ok = req.as_reader().take(1_048_576).read_to_string(&mut body).is_ok();
                let _ = req.respond(tiny_http::Response::from_string("{}").with_status_code(200));
                if read_ok {
                    if let Ok(v) = serde_json::from_str::<Value>(&body) {
                        on_body(v);
                    }
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
    use std::time::Duration;

    #[test]
    fn accepts_valid_token_and_rejects_others() {
        let (tx, rx) = std::sync::mpsc::channel();
        let server = HookServer::start(0, "abc".into(), move |v| {
            tx.send(v).unwrap();
        })
        .unwrap();
        let base = format!("http://127.0.0.1:{}", server.port);

        let ok = ureq::post(&format!("{base}/hook/abc"))
            .send_string(r#"{"hook_event_name":"Stop","session_id":"s1"}"#)
            .unwrap();
        assert_eq!(ok.status(), 200);
        let got = rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(got["session_id"], "s1");

        let bad = ureq::post(&format!("{base}/hook/wrong")).send_string("{}");
        assert!(matches!(bad, Err(ureq::Error::Status(404, _))));
        let get = ureq::get(&format!("{base}/hook/abc")).call();
        assert!(matches!(get, Err(ureq::Error::Status(404, _))));

        let not_json = ureq::post(&format!("{base}/hook/abc")).send_string("nope").unwrap();
        assert_eq!(not_json.status(), 200);
        assert!(rx.recv_timeout(Duration::from_millis(200)).is_err());

        server.stop();
    }

    #[test]
    fn busy_port_is_an_error() {
        let a = HookServer::start(0, "t".into(), |_| {}).unwrap();
        let err = HookServer::start(a.port, "t".into(), |_| {}).err().unwrap();
        assert!(err.contains(&a.port.to_string()), "{err}");
        a.stop();
    }
}
