//! Loopback receiver of the browser callback (issue #122): a one-shot HTTP
//! listener on `127.0.0.1` and a free port. The browser lands on
//! `/callback?code&state` (or `?error&state`) after Discord; the page it gets
//! back tells the user to return to the launcher.
use motogp_application::{CallbackParams, LoopbackError, LoopbackReceiver, LoopbackSession};
use std::io::{ErrorKind, Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::time::{Duration, Instant};

/// Largest request head read from the browser.
const MAX_REQUEST_BYTES: usize = 8 * 1024;
/// How long one connection may stay silent (browsers open speculative sockets).
const READ_TIMEOUT: Duration = Duration::from_secs(2);
const POLL_INTERVAL: Duration = Duration::from_millis(25);

const PAGE: &str = "<!doctype html><html><head><meta charset=\"utf-8\">\
<title>MotoGP Launcher</title></head><body style=\"font-family:sans-serif;\
text-align:center;margin-top:20vh\"><h1>You can return to the launcher.</h1>\
<p>You can close this tab.</p></body></html>";

/// [`LoopbackReceiver`] backed by a `std::net::TcpListener`.
#[derive(Debug, Default, Clone, Copy)]
pub struct TcpLoopbackReceiver;

impl LoopbackReceiver for TcpLoopbackReceiver {
    fn bind(&self) -> Result<Box<dyn LoopbackSession>, LoopbackError> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .map_err(|e| LoopbackError::Io(e.to_string()))?;
        listener
            .set_nonblocking(true)
            .map_err(|e| LoopbackError::Io(e.to_string()))?;
        let port = listener
            .local_addr()
            .map_err(|e| LoopbackError::Io(e.to_string()))?
            .port();
        Ok(Box::new(TcpLoopbackSession { listener, port }))
    }
}

struct TcpLoopbackSession {
    listener: TcpListener,
    port: u16,
}

impl LoopbackSession for TcpLoopbackSession {
    fn redirect_url(&self) -> String {
        format!("http://127.0.0.1:{}/callback", self.port)
    }

    fn wait(self: Box<Self>, timeout: Duration) -> Result<CallbackParams, LoopbackError> {
        let deadline = Instant::now() + timeout;
        loop {
            match self.listener.accept() {
                Ok((stream, _)) => {
                    if let Some(params) = handle(stream) {
                        return Ok(params);
                    }
                }
                Err(error) if error.kind() == ErrorKind::WouldBlock => {
                    if Instant::now() >= deadline {
                        return Err(LoopbackError::Timeout);
                    }
                    std::thread::sleep(POLL_INTERVAL);
                }
                Err(error) => return Err(LoopbackError::Io(error.to_string())),
            }
        }
    }
}

/// Answers one connection. `Some` only for a `GET /callback`; anything else
/// (favicon, probes, silence) gets a 404 and the wait goes on.
fn handle(mut stream: TcpStream) -> Option<CallbackParams> {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(READ_TIMEOUT));
    let head = read_head(&mut stream)?;
    let target = request_target(&head);
    let (path, query) = match target
        .as_deref()
        .map(|t| t.split_once('?').unwrap_or((t, "")))
    {
        Some(parts) => parts,
        None => {
            respond(&mut stream, "400 Bad Request", "");
            return None;
        }
    };
    if path != "/callback" {
        respond(&mut stream, "404 Not Found", "");
        return None;
    }
    respond(&mut stream, "200 OK", PAGE);
    Some(parse_query(query))
}

fn read_head(stream: &mut TcpStream) -> Option<String> {
    let mut raw = Vec::new();
    let mut chunk = [0u8; 1024];
    while raw.len() < MAX_REQUEST_BYTES && !raw.windows(4).any(|w| w == b"\r\n\r\n") {
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => raw.extend_from_slice(&chunk[..n]),
            Err(_) => break,
        }
    }
    if raw.is_empty() {
        return None;
    }
    Some(String::from_utf8_lossy(&raw).into_owned())
}

/// The target of a `GET` request line, or `None` for anything else.
fn request_target(head: &str) -> Option<String> {
    let line = head.lines().next()?;
    let mut parts = line.split_whitespace();
    if parts.next()? != "GET" {
        return None;
    }
    parts.next().map(str::to_string)
}

fn respond(stream: &mut TcpStream, status: &str, body: &str) {
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}

fn parse_query(query: &str) -> CallbackParams {
    let mut params = CallbackParams::default();
    for pair in query.split('&') {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        let value = percent_decode(value);
        match key {
            "code" => params.code = Some(value),
            "state" => params.state = Some(value),
            "error" => params.error = Some(value),
            _ => {}
        }
    }
    params
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len()
                && hex(bytes[i + 1]).is_some()
                && hex(bytes[i + 2]).is_some() =>
            {
                out.push(hex(bytes[i + 1]).unwrap_or(0) * 16 + hex(bytes[i + 2]).unwrap_or(0));
                i += 3;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            byte => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    /// Sends `request` to the session's port and returns the raw answer.
    fn browse(redirect: &str, request_line: &str) -> String {
        let port: u16 = redirect
            .trim_start_matches("http://127.0.0.1:")
            .split('/')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        stream
            .write_all(format!("{request_line}\r\nHost: 127.0.0.1\r\n\r\n").as_bytes())
            .unwrap();
        let mut answer = String::new();
        stream.read_to_string(&mut answer).unwrap();
        answer
    }

    fn bound() -> Box<dyn LoopbackSession> {
        TcpLoopbackReceiver.bind().unwrap()
    }

    #[test]
    fn the_redirect_is_a_loopback_callback_url() {
        let session = bound();
        let url = session.redirect_url();
        assert!(url.starts_with("http://127.0.0.1:"));
        assert!(url.ends_with("/callback"));
        let port: u16 = url
            .trim_start_matches("http://127.0.0.1:")
            .trim_end_matches("/callback")
            .parse()
            .unwrap();
        assert!(port >= 1024);
    }

    #[test]
    fn it_receives_code_and_state_and_answers_the_return_page() {
        let session = bound();
        let redirect = session.redirect_url();
        let browser = thread::spawn(move || {
            browse(&redirect, "GET /callback?code=abc%2D1&state=st_1 HTTP/1.1")
        });
        let params = session.wait(Duration::from_secs(5)).unwrap();
        assert_eq!(params.code.as_deref(), Some("abc-1"));
        assert_eq!(params.state.as_deref(), Some("st_1"));
        assert_eq!(params.error, None);
        let answer = browser.join().unwrap();
        assert!(answer.starts_with("HTTP/1.1 200 OK"));
        assert!(answer.contains("You can return to the launcher."));
    }

    #[test]
    fn it_receives_an_error_callback() {
        let session = bound();
        let redirect = session.redirect_url();
        let browser = thread::spawn(move || {
            browse(
                &redirect,
                "GET /callback?error=access_denied&state=st HTTP/1.1",
            )
        });
        let params = session.wait(Duration::from_secs(5)).unwrap();
        assert_eq!(params.error.as_deref(), Some("access_denied"));
        assert_eq!(params.state.as_deref(), Some("st"));
        assert_eq!(params.code, None);
        browser.join().unwrap();
    }

    #[test]
    fn other_paths_get_a_404_and_the_wait_goes_on() {
        let session = bound();
        let redirect = session.redirect_url();
        let browser = thread::spawn(move || {
            let favicon = browse(&redirect, "GET /favicon.ico HTTP/1.1");
            let post = browse(&redirect, "POST /callback?code=x&state=y HTTP/1.1");
            let real = browse(&redirect, "GET /callback?code=ok&state=s HTTP/1.1");
            (favicon, post, real)
        });
        let params = session.wait(Duration::from_secs(5)).unwrap();
        assert_eq!(params.code.as_deref(), Some("ok"));
        let (favicon, post, real) = browser.join().unwrap();
        assert!(favicon.starts_with("HTTP/1.1 404"));
        assert!(post.starts_with("HTTP/1.1 400"));
        assert!(real.starts_with("HTTP/1.1 200"));
    }

    #[test]
    fn it_times_out_when_nobody_comes_back() {
        let session = bound();
        assert_eq!(
            session.wait(Duration::from_millis(150)),
            Err(LoopbackError::Timeout)
        );
    }

    #[test]
    fn query_parsing_decodes_and_ignores_unknown_keys() {
        let params = parse_query("code=a%20b+c&x=1&state=%zz&error");
        assert_eq!(params.code.as_deref(), Some("a b c"));
        assert_eq!(params.state.as_deref(), Some("%zz"));
        assert_eq!(params.error.as_deref(), Some(""));
    }

    #[test]
    fn a_truncated_percent_escape_is_kept_literally() {
        assert_eq!(percent_decode("ab%4"), "ab%4");
        assert_eq!(percent_decode("%41"), "A");
    }
}
