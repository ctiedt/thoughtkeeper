use askama::Template;
use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
use comrak::Options;
use figment::{
    providers::{Format, Toml},
    Figment,
};
use itertools::Itertools;
use rss::{Guid, Item};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{Config, ServerConfig};

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
            published: Utc::now().naive_utc(),
        }
    }

    pub fn published(&self) -> String {
        self.published.format("%d.%m.%Y %H:%M").to_string()
    }

    pub fn teaser(&self) -> String {
        self.content.lines().take(5).join("\n")
    }

    pub fn url(&self) -> String {
        to_url(&self.title)
    }

    pub fn content(&self) -> String {
        let mut options = Options::default();
        options.extension.footnotes = true;
        options.extension.table = true;
        comrak::markdown_to_html(&self.content, &options)
    }
}

impl From<Article> for Item {
    fn from(article: Article) -> Self {
        let config: Config = Figment::new()
            .merge(Toml::file("blog.toml"))
            .extract()
            .unwrap();
        let server = config.server.unwrap();
        let domain = server.domain.unwrap();
        let content = article.content();

        let url = article.url();
        Item {
            title: Some(article.title),
            content: Some(content),
            author: Some(server.author),
            guid: Some(Guid {
                value: format!("https://{}/article/{}", &domain, url),
                permalink: true,
            }),
            link: Some(format!("https://{}/article/{}", &domain, url)),
            pub_date: Some(Utc.from_utc_datetime(&article.published).to_rfc2822()),
            ..Default::default()
        }
    }
}

pub fn to_url(title: &str) -> String {
    title
        .chars()
        .filter_map(|c| {
            if c.is_whitespace() {
                Some('_')
            } else if "-._~".contains(c) || c.is_alphanumeric() {
                Some(c)
            } else {
                None
            }
        })
        .collect()
}

#[derive(Clone, Template)]
#[template(path = "article.html")]
pub struct ArticleTemplate {
    pub config: ServerConfig,
    pub article: Article,
}
