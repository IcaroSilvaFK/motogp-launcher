//! Tiny scripted HTTP/1.1 server for tests, bound to 127.0.0.1 on a free port.
//! Tests never touch the internet. Each connection gets the next scripted
//! reply (the last one repeats) and every request head is recorded.
#![allow(dead_code)]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

/// One scripted response.
#[derive(Debug, Clone)]
pub struct Reply {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    /// `Content-Length` to announce when it differs from the body (a body
    /// shorter than this is a dropped connection).
    pub declared_len: Option<usize>,
}

impl Reply {
    pub fn new(status: u16) -> Self {
        Reply {
            status,
            headers: Vec::new(),
            body: Vec::new(),
            declared_len: None,
        }
    }

    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_string(), value.to_string()));
        self
    }

    pub fn declared_len(mut self, len: usize) -> Self {
        self.declared_len = Some(len);
        self
    }

    pub fn body(mut self, body: impl Into<Vec<u8>>) -> Self {
        self.body = body.into();
        self
    }
}

/// A request as the server saw it.
#[derive(Debug, Clone)]
pub struct Recorded {
    pub request_line: String,
    /// Header names lowercased.
    pub headers: Vec<(String, String)>,
}

impl Recorded {
    pub fn header(&self, name: &str) -> Option<&str> {
        let name = name.to_lowercase();
        self.headers
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, v)| v.as_str())
    }
}

#[derive(Debug)]
pub struct TestServer {
    pub url: String,
    requests: Arc<Mutex<Vec<Recorded>>>,
}

impl TestServer {
    /// Starts serving in a background thread; it ends with the test process.
    pub fn start(replies: Vec<Reply>) -> Self {
        assert!(!replies.is_empty(), "script at least one reply");
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind local port");
        let url = format!("http://{}/catalog.json", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&requests);
        std::thread::spawn(move || {
            for (index, stream) in listener.incoming().enumerate() {
                let Ok(mut stream) = stream else { continue };
                let reply = &replies[index.min(replies.len() - 1)];
                let recorded = read_request(&mut stream);
                log.lock().unwrap().push(recorded);
                let _ = write_reply(&mut stream, reply);
            }
        });
        TestServer { url, requests }
    }

    pub fn requests(&self) -> Vec<Recorded> {
        self.requests.lock().unwrap().clone()
    }
}

fn read_request(stream: &mut TcpStream) -> Recorded {
    let mut data = Vec::new();
    let mut byte = [0u8; 1];
    while !data.ends_with(b"\r\n\r\n") {
        if stream.read(&mut byte).unwrap_or(0) == 0 {
            break;
        }
        data.push(byte[0]);
    }
    let text = String::from_utf8_lossy(&data).to_string();
    let mut lines = text.split("\r\n");
    let request_line = lines.next().unwrap_or_default().to_string();
    let headers = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(n, v)| (n.trim().to_lowercase(), v.trim().to_string()))
        .collect();
    Recorded {
        request_line,
        headers,
    }
}

fn write_reply(stream: &mut TcpStream, reply: &Reply) -> std::io::Result<()> {
    let mut head = format!("HTTP/1.1 {} Scripted\r\n", reply.status);
    for (name, value) in &reply.headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str(&format!(
        "Content-Length: {}\r\nConnection: close\r\n\r\n",
        reply.declared_len.unwrap_or(reply.body.len())
    ));
    stream.write_all(head.as_bytes())?;
    stream.write_all(&reply.body)?;
    stream.flush()
}
