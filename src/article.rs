use askama::Template;
use chrono::{NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::ServerConfig;

#[derive(Clone, Serialize, Deserialize)]
pub struct Article {
    pub id: String,
    pub title: String,
    pub content: String,
    pub published: NaiveDateTime,
}

impl Article {
    pub fn new(title: String, content: String) -> Self {
        Article {
            id: Uuid::new_v4().to_string(),
            title,
            content,
            published: Utc::now().naive_local(),
        }
    }
}

#[derive(Clone, Template)]
#[template(path = "article.html")]
pub struct ArticleTemplate {
    pub config: ServerConfig,
    pub article: Article,
}
