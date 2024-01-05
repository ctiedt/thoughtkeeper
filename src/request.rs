use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

use crate::article::Article;

#[derive(Serialize, Deserialize)]
pub struct Request {
    pub secret: String,
    pub request: InnerRequest,
}

#[derive(Serialize, Deserialize)]
pub enum InnerRequest {
    CreateArticle {
        title: String,
        content: String,
    },
    GetArticle {
        url: String,
    },
    YankArticle {
        id: String,
    },
    UpdateArticle {
        id: String,
        title: Option<String>,
        content: Option<String>,
    },
    ListArticles,
}

#[derive(Serialize, Deserialize)]
pub struct ArticleMetadata {
    pub id: String,
    pub title: String,
    pub published: NaiveDateTime,
}

#[derive(Serialize, Deserialize)]
pub enum Response {
    Article(Article),
    ArticleId(String),
    ArticleMetadata(Vec<ArticleMetadata>),
    Untyped { kind: String, content: String },
    Ok,
    Error(String),
}
