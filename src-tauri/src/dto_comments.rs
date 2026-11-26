//! IPC shapes of the mod comments (#118). camelCase, and no field can hold the
//! session token: the DTOs are built only from the domain's comment types.
use motogp_domain::{CommentsPage, ModComment};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModCommentDto {
    pub id: String,
    pub author_name: String,
    /// A `data:` URL, or `null` to show initials.
    pub author_avatar_data_url: Option<String>,
    /// Plain text: the UI renders it with the safe Markdown renderer.
    pub body: String,
    /// Milliseconds since the Unix epoch.
    pub created_at_ms: i64,
    pub edited: bool,
    pub own: bool,
}

impl From<ModComment> for ModCommentDto {
    fn from(comment: ModComment) -> Self {
        Self {
            id: comment.id,
            author_name: comment.author_name,
            author_avatar_data_url: comment.author_avatar_data_url,
            body: comment.body,
            created_at_ms: comment.created_at_ms,
            edited: comment.edited,
            own: comment.own,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentsPageDto {
    pub comments: Vec<ModCommentDto>,
    pub next_cursor: Option<String>,
}

impl From<CommentsPage> for CommentsPageDto {
    fn from(page: CommentsPage) -> Self {
        Self {
            comments: page.comments.into_iter().map(Into::into).collect(),
            next_cursor: page.next_cursor,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn comment() -> ModComment {
        ModComment {
            id: "c1".into(),
            author_name: "Ada".into(),
            author_avatar_data_url: None,
            body: "hi".into(),
            created_at_ms: 1_000,
            edited: true,
            own: false,
        }
    }

    #[test]
    fn a_page_serializes_in_camel_case() {
        let page = CommentsPageDto::from(CommentsPage {
            comments: vec![comment()],
            next_cursor: Some("n".into()),
        });
        assert_eq!(
            serde_json::to_value(page).unwrap(),
            json!({
                "comments": [{
                    "id": "c1",
                    "authorName": "Ada",
                    "authorAvatarDataUrl": null,
                    "body": "hi",
                    "createdAtMs": 1000,
                    "edited": true,
                    "own": false
                }],
                "nextCursor": "n"
            })
        );
    }
}
