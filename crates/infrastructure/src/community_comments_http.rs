//! HTTP adapter of the community server's comment endpoints (issue #118).
//!
//! Reuses the account client of [`HttpCommunityApi`]. The session token only
//! ever goes into the `Authorization` header; it is never logged and never part
//! of an error message. Path segments are percent-encoded and answers are
//! capped at 64 KiB.
use crate::community_http::{describe, percent_encode, MAX_BODY_BYTES};
use crate::HttpCommunityApi;
use motogp_application::{ApiComment, ApiCommentsPage, CommentsApiError, CommunityCommentsApi};
use motogp_domain::SessionToken;
use reqwest::blocking::{RequestBuilder, Response};
use reqwest::header::{CONTENT_TYPE, RETRY_AFTER};
use reqwest::StatusCode;
use serde::Deserialize;
use std::io::Read;

#[derive(Deserialize)]
struct AuthorWire {
    display_name: String,
    avatar_url: Option<String>,
}

#[derive(Deserialize)]
struct CommentWire {
    id: String,
    body: String,
    created_at: String,
    edited_at: Option<String>,
    author: AuthorWire,
    own: bool,
}

#[derive(Deserialize)]
struct PageWire {
    comments: Vec<CommentWire>,
    next_cursor: Option<String>,
}

impl CommentWire {
    fn into_comment(self) -> Result<ApiComment, CommentsApiError> {
        let created_at_ms = parse_rfc3339_ms(&self.created_at)
            .ok_or_else(|| CommentsApiError::Protocol("bad created_at".into()))?;
        Ok(ApiComment {
            id: self.id,
            author_name: self.author.display_name,
            author_avatar_url: self.author.avatar_url,
            body: self.body,
            created_at_ms,
            edited: self.edited_at.is_some(),
            own: self.own,
        })
    }
}

/// Milliseconds since the Unix epoch of an RFC 3339 timestamp (`Z` or a
/// `+hh:mm` offset, optional fraction). `None` for anything else.
pub(crate) fn parse_rfc3339_ms(text: &str) -> Option<i64> {
    let bytes = text.as_bytes();
    if bytes.len() < 20 || !text.is_ascii() {
        return None;
    }
    let num = |from: usize, to: usize| -> Option<i64> {
        let part = text.get(from..to)?;
        part.bytes()
            .all(|b| b.is_ascii_digit())
            .then(|| part.parse().ok())?
    };
    if bytes[4] != b'-'
        || bytes[7] != b'-'
        || !matches!(bytes[10], b'T' | b't')
        || bytes[13] != b':'
        || bytes[16] != b':'
    {
        return None;
    }
    let (year, month, day) = (num(0, 4)?, num(5, 7)?, num(8, 10)?);
    let (hour, minute, second) = (num(11, 13)?, num(14, 16)?, num(17, 19)?);
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 60
    {
        return None;
    }
    let mut at = 19;
    let mut millis = 0;
    if bytes[at] == b'.' {
        let start = at + 1;
        at = start;
        while at < bytes.len() && bytes[at].is_ascii_digit() {
            at += 1;
        }
        if at == start {
            return None;
        }
        let digits = &text[start..at.min(start + 3)];
        millis = digits.parse::<i64>().ok()? * 10_i64.pow(3 - digits.len() as u32);
    }
    let offset_minutes = match text.get(at..)? {
        "Z" | "z" => 0,
        rest if rest.len() == 6 && (rest.starts_with('+') || rest.starts_with('-')) => {
            let hh: i64 = rest[1..3].parse().ok()?;
            let mm: i64 = rest[4..6].parse().ok()?;
            if &rest[3..4] != ":" || hh > 23 || mm > 59 {
                return None;
            }
            let sign = if rest.starts_with('-') { -1 } else { 1 };
            sign * (hh * 60 + mm)
        }
        _ => return None,
    };
    // Days since 1970-01-01 (Howard Hinnant's civil-from-days, inverted).
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    let seconds = days * 86_400 + hour * 3_600 + minute * 60 + second - offset_minutes * 60;
    Some(seconds * 1_000 + millis)
}

fn send(request: RequestBuilder) -> Result<Response, CommentsApiError> {
    request
        .send()
        .map_err(|e| CommentsApiError::Unreachable(describe(&e)))
}

/// Maps a non-success status to the port's error type.
fn status_error(response: &Response) -> CommentsApiError {
    let status = response.status();
    match status {
        StatusCode::BAD_REQUEST => CommentsApiError::Invalid,
        StatusCode::UNAUTHORIZED => CommentsApiError::Unauthorized,
        StatusCode::NOT_FOUND => CommentsApiError::NotFound,
        StatusCode::TOO_MANY_REQUESTS => {
            let retry_after_secs = response
                .headers()
                .get(RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.trim().parse::<u64>().ok())
                .unwrap_or(1)
                .max(1);
            CommentsApiError::RateLimited { retry_after_secs }
        }
        s if s.is_server_error() => CommentsApiError::Unreachable(format!("HTTP {}", s.as_u16())),
        s => CommentsApiError::Protocol(format!("HTTP {}", s.as_u16())),
    }
}

fn ok(response: Response) -> Result<Response, CommentsApiError> {
    if response.status().is_success() {
        Ok(response)
    } else {
        Err(status_error(&response))
    }
}

/// Reads at most 64 KiB and parses JSON.
fn read_json<T: for<'de> Deserialize<'de>>(response: Response) -> Result<T, CommentsApiError> {
    let mut bytes = Vec::new();
    response
        .take(MAX_BODY_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| CommentsApiError::Unreachable("could not read the answer".into()))?;
    if bytes.len() as u64 > MAX_BODY_BYTES {
        return Err(CommentsApiError::Protocol("answer too large".into()));
    }
    serde_json::from_slice(&bytes)
        .map_err(|_| CommentsApiError::Protocol("answer is not the expected JSON".into()))
}

fn comments_path(mod_id: &str) -> String {
    format!("/mods/{}/comments", percent_encode(mod_id))
}

fn comment_path(mod_id: &str, comment_id: &str) -> String {
    format!("{}/{}", comments_path(mod_id), percent_encode(comment_id))
}

fn json_body(body: &str) -> String {
    serde_json::json!({ "body": body }).to_string()
}

impl CommunityCommentsApi for HttpCommunityApi {
    fn list(
        &self,
        mod_id: &str,
        cursor: Option<&str>,
        token: Option<&SessionToken>,
    ) -> Result<ApiCommentsPage, CommentsApiError> {
        let mut url = self.url(&comments_path(mod_id));
        if let Some(cursor) = cursor {
            url.push_str(&format!("?cursor={}", percent_encode(cursor)));
        }
        let mut request = self.client.get(url);
        if let Some(token) = token {
            request = request.bearer_auth(token.expose());
        }
        let page: PageWire = read_json(ok(send(request)?)?)?;
        let comments = page
            .comments
            .into_iter()
            .map(CommentWire::into_comment)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ApiCommentsPage {
            comments,
            next_cursor: page.next_cursor,
        })
    }

    fn post(
        &self,
        mod_id: &str,
        body: &str,
        token: &SessionToken,
    ) -> Result<ApiComment, CommentsApiError> {
        let response = send(
            self.client
                .post(self.url(&comments_path(mod_id)))
                .bearer_auth(token.expose())
                .header(CONTENT_TYPE, "application/json")
                .body(json_body(body)),
        )?;
        read_json::<CommentWire>(ok(response)?)?.into_comment()
    }

    fn edit(
        &self,
        mod_id: &str,
        comment_id: &str,
        body: &str,
        token: &SessionToken,
    ) -> Result<ApiComment, CommentsApiError> {
        let response = send(
            self.client
                .patch(self.url(&comment_path(mod_id, comment_id)))
                .bearer_auth(token.expose())
                .header(CONTENT_TYPE, "application/json")
                .body(json_body(body)),
        )?;
        read_json::<CommentWire>(ok(response)?)?.into_comment()
    }

    fn delete(
        &self,
        mod_id: &str,
        comment_id: &str,
        token: &SessionToken,
    ) -> Result<(), CommentsApiError> {
        let response = send(
            self.client
                .delete(self.url(&comment_path(mod_id, comment_id)))
                .bearer_auth(token.expose()),
        )?;
        ok(response).map(drop)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::community_http::tests::serve_once;
    use crate::HttpCommunityConfig;

    fn api(base: &str) -> HttpCommunityApi {
        HttpCommunityApi::new(HttpCommunityConfig::new(base, "0.0.0")).unwrap()
    }

    const COMMENT: &str = r#"{"id":"c1","body":"Great livery","created_at":"2026-11-26T10:15:30.123Z","edited_at":null,"author":{"id":"u","display_name":"Ada","avatar_url":"https://cdn.example/a.png"},"own":true}"#;

    fn token() -> SessionToken {
        SessionToken::new("tok-SECRET-123")
    }

    #[test]
    fn timestamps_parse_to_epoch_milliseconds() {
        assert_eq!(parse_rfc3339_ms("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_rfc3339_ms("1970-01-01T00:00:01.5Z"), Some(1_500));
        assert_eq!(
            parse_rfc3339_ms("2026-11-26T10:15:30.123Z"),
            Some(1_795_688_130_123)
        );
        // Offsets shift to UTC; a leap day works; extra fraction digits are cut.
        assert_eq!(
            parse_rfc3339_ms("2026-11-26T12:15:30.123+02:00"),
            Some(1_795_688_130_123)
        );
        assert_eq!(
            parse_rfc3339_ms("2000-02-29T00:00:00Z"),
            Some(951_782_400_000)
        );
        assert_eq!(parse_rfc3339_ms("1970-01-01T00:00:00.123456Z"), Some(123));
    }

    #[test]
    fn malformed_timestamps_are_rejected() {
        for bad in [
            "",
            "yesterday",
            "2026-11-26",
            "2026-13-26T10:15:30Z",
            "2026-11-26T25:15:30Z",
            "2026-11-26 10:15:30Z",
            "2026-11-26T10:15:30",
            "2026-11-26T10:15:30.Z",
            "2026-11-26T10:15:30+0200",
            "2026-11-26T10:15:30Zjunk",
            "2026-11-2éT10:15:30Z",
        ] {
            assert_eq!(parse_rfc3339_ms(bad), None, "{bad}");
        }
    }

    #[test]
    fn list_without_a_token_sends_no_authorization_and_parses_the_page() {
        let body = format!(r#"{{"comments":[{COMMENT}],"next_cursor":"cur2"}}"#);
        let (base, requests) = serve_once("200 OK", &body);
        let page = api(&base).list("red-livery", None, None).unwrap();
        let request = requests.recv().unwrap();
        assert!(request.starts_with("GET /mods/red-livery/comments "));
        assert!(!request.to_ascii_lowercase().contains("authorization"));
        assert_eq!(page.next_cursor.as_deref(), Some("cur2"));
        let c = &page.comments[0];
        assert_eq!(c.id, "c1");
        assert_eq!(c.author_name, "Ada");
        assert_eq!(
            c.author_avatar_url.as_deref(),
            Some("https://cdn.example/a.png")
        );
        assert_eq!(c.created_at_ms, 1_795_688_130_123);
        assert!(!c.edited);
        assert!(c.own);
    }

    #[test]
    fn list_sends_the_encoded_cursor_and_the_token() {
        let (base, requests) = serve_once("200 OK", r#"{"comments":[],"next_cursor":null}"#);
        let page = api(&base)
            .list("livery", Some("a b/c"), Some(&token()))
            .unwrap();
        assert!(page.comments.is_empty() && page.next_cursor.is_none());
        let request = requests.recv().unwrap();
        assert!(request.starts_with("GET /mods/livery/comments?cursor=a%20b%2Fc "));
        assert!(request.contains("tok-SECRET-123"));
    }

    #[test]
    fn an_edited_comment_is_marked_edited() {
        let body = COMMENT.replace(
            r#""edited_at":null"#,
            r#""edited_at":"2026-11-26T11:00:00Z""#,
        );
        let (base, _) = serve_once("200 OK", &body);
        let edited = api(&base).edit("livery", "c1", "x", &token()).unwrap();
        assert!(edited.edited);
    }

    #[test]
    fn post_sends_json_and_the_bearer_and_expects_the_comment() {
        let (base, requests) = serve_once("201 Created", COMMENT);
        let comment = api(&base)
            .post("livery", "Great \"livery\"", &token())
            .unwrap();
        assert_eq!(comment.id, "c1");
        let request = requests.recv().unwrap();
        assert!(request.starts_with("POST /mods/livery/comments "));
        assert!(request.contains("tok-SECRET-123"));
        assert!(request.contains(r#"{"body":"Great \"livery\""}"#));
    }

    #[test]
    fn edit_patches_and_delete_deletes_the_encoded_comment_path() {
        let (base, requests) = serve_once("200 OK", COMMENT);
        api(&base).edit("livery", "a/b", "x", &token()).unwrap();
        assert!(requests
            .recv()
            .unwrap()
            .starts_with("PATCH /mods/livery/comments/a%2Fb "));
        let (base, requests) = serve_once("204 No Content", "");
        api(&base).delete("livery", "c1", &token()).unwrap();
        assert!(requests
            .recv()
            .unwrap()
            .starts_with("DELETE /mods/livery/comments/c1 "));
    }

    #[test]
    fn statuses_map_to_typed_errors() {
        let cases = [
            ("400 Bad Request", CommentsApiError::Invalid),
            ("401 Unauthorized", CommentsApiError::Unauthorized),
            ("404 Not Found", CommentsApiError::NotFound),
            (
                "429 Too Many Requests\r\nRetry-After: 12",
                CommentsApiError::RateLimited {
                    retry_after_secs: 12,
                },
            ),
            (
                "429 Too Many Requests",
                CommentsApiError::RateLimited {
                    retry_after_secs: 1,
                },
            ),
            (
                "503 Service Unavailable",
                CommentsApiError::Unreachable("HTTP 503".into()),
            ),
            (
                "418 I'm a teapot",
                CommentsApiError::Protocol("HTTP 418".into()),
            ),
        ];
        for (status, expected) in cases {
            let (base, _) = serve_once(status, r#"{"error":{"code":"x","message":"y"}}"#);
            assert_eq!(
                api(&base).delete("livery", "c1", &token()),
                Err(expected),
                "{status}"
            );
        }
    }

    #[test]
    fn junk_answers_are_protocol_errors_and_never_leak_the_token() {
        let (base, _) = serve_once("200 OK", "<html>nope</html>");
        let error = api(&base).list("livery", None, Some(&token())).unwrap_err();
        assert!(matches!(error, CommentsApiError::Protocol(_)));
        assert!(!format!("{error:?}{error}").contains("tok-SECRET-123"));
        let (base, _) = serve_once(
            "200 OK",
            &COMMENT.replace("2026-11-26T10:15:30.123Z", "soon"),
        );
        assert!(matches!(
            api(&base).post("livery", "x", &token()),
            Err(CommentsApiError::Protocol(_))
        ));
    }

    #[test]
    fn an_oversized_answer_is_rejected() {
        let big = format!(
            r#"{{"comments":[],"next_cursor":"{}"}}"#,
            "a".repeat(70 * 1024)
        );
        let (base, _) = serve_once("200 OK", &big);
        assert!(matches!(
            api(&base).list("livery", None, None),
            Err(CommentsApiError::Protocol(_))
        ));
    }

    #[test]
    fn an_unreachable_server_never_mentions_the_token() {
        let api = api("http://127.0.0.1:1");
        let error = api.post("livery", "x", &token()).unwrap_err();
        assert!(matches!(error, CommentsApiError::Unreachable(_)));
        assert!(!error.to_string().contains("tok-SECRET-123"));
    }
}
