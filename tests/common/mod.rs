//! Shared test helpers.

#![allow(dead_code)] // each test crate uses a different subset

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;

/// One HTTP request seen by the mock server
#[derive(Debug, Clone)]
pub struct Request {
    pub method: String,
    /// Path and query string, e.g. `/api/get?artist_name=Queen`
    pub target: String,
    pub body: String,
}

type Handler = Box<dyn Fn(&Request) -> (u16, String) + Send + Sync>;

/// A tiny local HTTP server answering with canned or computed JSON, recording what it was asked.
/// Routes are matched in order on `"METHOD /path-prefix"`; anything else is a 404.
pub struct MockServer {
    /// `http://127.0.0.1:<port>`
    pub url: String,
    requests: Arc<Mutex<Vec<Request>>>,
}

impl MockServer {
    pub fn start(routes: Vec<(&'static str, Handler)>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
        let url = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let routes = Arc::new(routes);

        let seen = requests.clone();
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let routes = routes.clone();
                let seen = seen.clone();
                thread::spawn(move || serve_one(stream, &routes, &seen));
            }
        });

        Self { url, requests }
    }

    /// Requests received so far
    pub fn requests(&self) -> Vec<Request> {
        self.requests.lock().unwrap().clone()
    }

    /// Requests whose `"METHOD target"` starts with `prefix`
    pub fn count(&self, prefix: &str) -> usize {
        self.requests()
            .iter()
            .filter(|r| format!("{} {}", r.method, r.target).starts_with(prefix))
            .count()
    }
}

/// Route that always answers with `status` and `body`
pub fn fixed(status: u16, body: &str) -> Handler {
    let body = body.to_string();
    Box::new(move |_| (status, body.clone()))
}

fn serve_one(
    mut stream: std::net::TcpStream,
    routes: &[(&'static str, Handler)],
    seen: &Mutex<Vec<Request>>,
) {
    let mut raw = Vec::new();
    let mut chunk = [0u8; 4096];
    let (head_end, content_length) = loop {
        let Ok(n) = stream.read(&mut chunk) else { return };
        if n == 0 {
            return;
        }
        raw.extend_from_slice(&chunk[..n]);
        if let Some(pos) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&raw[..pos]).to_lowercase();
            let length = head
                .lines()
                .find_map(|l| l.strip_prefix("content-length:"))
                .and_then(|v| v.trim().parse::<usize>().ok())
                .unwrap_or(0);
            break (pos + 4, length);
        }
    };
    while raw.len() < head_end + content_length {
        let Ok(n) = stream.read(&mut chunk) else { return };
        if n == 0 {
            break;
        }
        raw.extend_from_slice(&chunk[..n]);
    }

    let head = String::from_utf8_lossy(&raw[..head_end]).to_string();
    let mut parts = head.lines().next().unwrap_or("").split_whitespace();
    let request = Request {
        method: parts.next().unwrap_or("").to_string(),
        target: parts.next().unwrap_or("").to_string(),
        body: String::from_utf8_lossy(&raw[head_end..]).to_string(),
    };
    seen.lock().unwrap().push(request.clone());

    let key = format!("{} {}", request.method, request.target);
    let (status, body) = routes
        .iter()
        .find(|(prefix, _)| key.starts_with(prefix))
        .map_or((404, "{}".to_string()), |(_, handler)| handler(&request));

    let response = format!(
        "HTTP/1.1 {status} Mock\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
}
