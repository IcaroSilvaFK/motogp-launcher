//! Avatar download for the community account row (issue #122). The WebView CSP
//! only allows `data:` images, so the Rust side fetches the avatar and hands it
//! over as a `data:` URL. Any failure is `None` (the row shows initials).
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use motogp_application::AvatarFetcher;
use reqwest::blocking::Client;
use reqwest::header::CONTENT_TYPE;
use std::io::Read;
use std::time::Duration;

/// Largest accepted avatar, in bytes.
pub const MAX_AVATAR_BYTES: u64 = 256 * 1024;

const ALLOWED_TYPES: [&str; 4] = ["image/png", "image/jpeg", "image/webp", "image/gif"];

/// [`AvatarFetcher`] over HTTP(S).
#[derive(Debug)]
pub struct HttpAvatarFetcher {
    client: Client,
}

impl HttpAvatarFetcher {
    /// Builds the HTTP client (5 s to connect, 10 s in total).
    pub fn new(app_version: &str) -> Result<Self, String> {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let client = Client::builder()
            .user_agent(format!("motogp-launcher/{app_version}"))
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self { client })
    }
}

/// Only https, or plain http to the local machine (tests, local server).
fn is_allowed_url(url: &str) -> bool {
    url.starts_with("https://")
        || url.starts_with("http://127.0.0.1:")
        || url.starts_with("http://127.0.0.1/")
}

impl AvatarFetcher for HttpAvatarFetcher {
    fn fetch(&self, url: &str) -> Option<String> {
        if !is_allowed_url(url) {
            return None;
        }
        let response = self.client.get(url).send().ok()?;
        if !response.status().is_success() {
            return None;
        }
        if response
            .content_length()
            .is_some_and(|len| len > MAX_AVATAR_BYTES)
        {
            return None;
        }
        let mime = response
            .headers()
            .get(CONTENT_TYPE)?
            .to_str()
            .ok()?
            .split(';')
            .next()?
            .trim()
            .to_ascii_lowercase();
        if !ALLOWED_TYPES.contains(&mime.as_str()) {
            return None;
        }
        // One extra byte proves the body is too big.
        let mut bytes = Vec::new();
        response
            .take(MAX_AVATAR_BYTES + 1)
            .read_to_end(&mut bytes)
            .ok()?;
        if bytes.is_empty() || bytes.len() as u64 > MAX_AVATAR_BYTES {
            return None;
        }
        Some(format!("data:{mime};base64,{}", STANDARD.encode(bytes)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::net::TcpListener;
    use std::thread;

    fn serve(content_type: &str, body: Vec<u8>, status: &str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/a.png", listener.local_addr().unwrap());
        let head = format!(
            "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = [0u8; 2048];
            let _ = stream.read(&mut buffer);
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(&body);
        });
        url
    }

    fn fetcher() -> HttpAvatarFetcher {
        HttpAvatarFetcher::new("0.0.0").unwrap()
    }

    #[test]
    fn a_small_image_becomes_a_data_url() {
        let url = serve("image/png", vec![1, 2, 3], "200 OK");
        assert_eq!(
            fetcher().fetch(&url).as_deref(),
            Some("data:image/png;base64,AQID")
        );
    }

    #[test]
    fn content_type_parameters_and_case_are_tolerated() {
        let url = serve("Image/WebP; charset=binary", vec![1, 2, 3], "200 OK");
        assert_eq!(
            fetcher().fetch(&url).as_deref(),
            Some("data:image/webp;base64,AQID")
        );
    }

    #[test]
    fn every_allowed_type_passes() {
        for kind in ["image/jpeg", "image/gif"] {
            let url = serve(kind, vec![9], "200 OK");
            assert!(fetcher().fetch(&url).unwrap().starts_with("data:image/"));
        }
    }

    #[test]
    fn other_types_are_refused() {
        for kind in ["image/svg+xml", "text/html", "application/octet-stream"] {
            let url = serve(kind, vec![1, 2, 3], "200 OK");
            assert_eq!(fetcher().fetch(&url), None, "{kind}");
        }
    }

    #[test]
    fn an_oversized_image_is_refused() {
        let url = serve(
            "image/png",
            vec![0; MAX_AVATAR_BYTES as usize + 1],
            "200 OK",
        );
        assert_eq!(fetcher().fetch(&url), None);
    }

    #[test]
    fn an_image_at_the_limit_is_accepted() {
        let url = serve("image/png", vec![0; MAX_AVATAR_BYTES as usize], "200 OK");
        assert!(fetcher().fetch(&url).is_some());
    }

    #[test]
    fn an_http_error_gives_none() {
        let url = serve("image/png", vec![1], "404 Not Found");
        assert_eq!(fetcher().fetch(&url), None);
    }

    #[test]
    fn an_empty_body_gives_none() {
        let url = serve("image/png", vec![], "200 OK");
        assert_eq!(fetcher().fetch(&url), None);
    }

    #[test]
    fn an_unreachable_server_gives_none() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/a.png", listener.local_addr().unwrap());
        drop(listener);
        assert_eq!(fetcher().fetch(&url), None);
    }

    #[test]
    fn unsafe_schemes_and_hosts_are_not_even_requested() {
        for url in [
            "http://example.com/a.png",
            "file:///c:/a.png",
            "ftp://x/a.png",
            "",
        ] {
            assert_eq!(fetcher().fetch(url), None, "{url}");
        }
    }
}
