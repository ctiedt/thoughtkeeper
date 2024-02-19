use chrono::{NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Clone)]
pub struct Comment {
    pub id: String,
    pub article: String,
    pub author: String,
    pub content: String,
    pub published: NaiveDateTime,
}

impl Comment {
    pub fn from_request(req: CommentRequest) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            article: req.article,
            author: req.author,
            content: req.content,
            published: Utc::now().naive_utc(),
        }
    }

    pub fn published(&self) -> String {
        self.published.format("%d.%m.%Y %H:%M").to_string()
    }
}

#[derive(Serialize, Deserialize)]
pub struct CommentRequest {
    article: String,
    author: String,
    content: String,
}
