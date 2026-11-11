//! Transport layer — HTTP/1.1 by hand over `std::net`, no dependencies.
//!
//! Deliberately ignorant of the simulation: it parses a request and hands it to a
//! caller-supplied handler. That keeps B6 `ws` swappable — the register rules
//! WebSocket but its own rationale defers it until measured, so the demo polls
//! behind this seam rather than hard-wiring a transport into the game.
//!
//! One connection at a time, `Connection: close`, with read/write timeouts so a
//! wedged client cannot block the accept loop. Same shape as the Pong POC, which
//! measured 666 req/s single-threaded with zero RSS growth.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;

pub struct Request {
    pub method: String,
    pub path: String,
    /// Raw query string without the `?`, empty when absent.
    pub query: String,
}

impl Request {
    /// Value of `key` in the query string, if present.
    pub fn param(&self, key: &str) -> Option<&str> {
        self.query.split('&').find_map(|kv| {
            let (k, v) = kv.split_once('=')?;
            (k == key).then_some(v)
        })
    }
}

pub struct Response {
    pub status: &'static str,
    pub content_type: &'static str,
    pub body: String,
}

impl Response {
    pub fn html(body: String) -> Response {
        Response {
            status: "200 OK",
            content_type: "text/html; charset=utf-8",
            body,
        }
    }

    pub fn json(body: String) -> Response {
        Response {
            status: "200 OK",
            content_type: "application/json",
            body,
        }
    }

    pub fn not_found() -> Response {
        Response {
            status: "404 Not Found",
            content_type: "text/plain; charset=utf-8",
            body: "not found".to_string(),
        }
    }

    pub fn bad_request(why: &str) -> Response {
        Response {
            status: "400 Bad Request",
            content_type: "text/plain; charset=utf-8",
            body: why.to_string(),
        }
    }
}

/// Serve forever, or until the accept loop fails. `handler` is `FnMut` because
/// the caller's world is mutated by requests.
pub fn serve<F>(addr: &str, mut handler: F) -> std::io::Result<()>
where
    F: FnMut(&Request) -> Response,
{
    let listener = TcpListener::bind(addr)?;
    eprintln!("elysia listening on http://{addr}/");
    for stream in listener.incoming() {
        match stream {
            Ok(mut s) => {
                let _ = s.set_read_timeout(Some(Duration::from_secs(5)));
                let _ = s.set_write_timeout(Some(Duration::from_secs(5)));
                if let Err(e) = handle(&mut s, &mut handler) {
                    eprintln!("conn error: {e}");
                }
            }
            Err(e) => eprintln!("accept error: {e}"),
        }
    }
    Ok(())
}

fn handle<F>(s: &mut TcpStream, handler: &mut F) -> std::io::Result<()>
where
    F: FnMut(&Request) -> Response,
{
    let mut buf = [0u8; 4096];
    let n = s.read(&mut buf)?;
    if n == 0 {
        return Ok(());
    }
    let raw = String::from_utf8_lossy(&buf[..n]);
    let line = raw.lines().next().unwrap_or("");

    // Request line is "<METHOD> <TARGET> <VERSION>".
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let target = parts.next().unwrap_or("/");
    let (path, query) = match target.split_once('?') {
        Some((p, q)) => (p.to_string(), q.to_string()),
        None => (target.to_string(), String::new()),
    };

    let req = Request {
        method,
        path,
        query,
    };
    let resp = handler(&req);

    let head = format!(
        "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        resp.status,
        resp.content_type,
        resp.body.len()
    );
    s.write_all(head.as_bytes())?;
    s.write_all(resp.body.as_bytes())?;
    s.flush()
}
