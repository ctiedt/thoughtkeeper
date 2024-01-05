use askama::Template;
use askama_axum::IntoResponse;
use axum::{
    extract::{Path, State},
    http::header,
    response::Response as AxumResponse,
    routing::{get, get_service, post},
    Json, Router,
};

use itertools::Itertools;
use miette::IntoDiagnostic;

use rss::ChannelBuilder;
use sqlx::{
    pool::PoolConnection, sqlite::SqliteConnectOptions, ConnectOptions, Pool, Sqlite,
    SqliteConnection, SqlitePool,
};
use tokio::net::TcpListener;
use tower_http::services::{ServeDir, ServeFile};

use crate::{
    article::{to_url, Article, ArticleTemplate},
    error::TkError,
    request::{ArticleMetadata, InnerRequest, Request, Response},
    ServerConfig,
};
use comfy_table::{Row, Table};
use rand::{
    distributions::{Alphanumeric, DistString},
    thread_rng,
};
use std::str::FromStr;

#[derive(Clone)]
struct BlogState {
    pool: Pool<Sqlite>,
    config: ServerConfig,
}

impl BlogState {
    async fn get_conn(&self) -> PoolConnection<Sqlite> {
        self.pool.acquire().await.unwrap()
    }
}

async fn handle_api_request(
    State(state): State<BlogState>,
    Json(request): Json<Request>,
) -> Result<AxumResponse, TkError> {
    let mut conn = state.get_conn().await;

    if !is_secret_valid(&request.secret, &mut conn).await? {
        return Ok(Json(Response::Error("Invalid secret".to_string())).into_response());
    }

    match request.request {
        InnerRequest::CreateArticle { title, content } => {
            let article = Article::new(title, content);

            sqlx::query!(
                "INSERT INTO articles ( id, title, content, published ) VALUES (?1, ?2, ?3, ?4)",
                article.id,
                article.title,
                article.content,
                article.published
            )
            .execute(&mut *conn)
            .await
            .into_diagnostic()?;

            Ok(Json(Response::ArticleId(article.id)).into_response())
        }
        InnerRequest::GetArticle { url } => {
            let titles = sqlx::query!("SELECT id, title FROM articles")
                .fetch_all(&mut *conn)
                .await
                .into_diagnostic()?;

            let id = titles
                .iter()
                .find_map(|r| {
                    if to_url(&r.title) == url {
                        Some(&r.id)
                    } else {
                        None
                    }
                })
                .ok_or(miette::miette!("No article with url {url} found"))?;
            let article = sqlx::query_as!(Article, "SELECT * FROM articles WHERE id = ?", id)
                .fetch_one(&mut *conn)
                .await
                .into_diagnostic()?;

            Ok(Json(serde_json::to_string(&article).into_diagnostic()?).into_response())
        }
        InnerRequest::YankArticle { id } => {
            sqlx::query!("DELETE FROM articles WHERE id = ?", id)
                .execute(&mut *conn)
                .await
                .into_diagnostic()?;

            Ok(Json(Response::Ok).into_response())
        }
        InnerRequest::ListArticles => {
            let articles = sqlx::query!("SELECT id, title, published FROM articles")
                .fetch_all(&mut *conn)
                .await
                .into_diagnostic()?;

            Ok(Json(Response::ArticleMetadata(
                articles
                    .into_iter()
                    .map(|r| ArticleMetadata {
                        id: r.id,
                        title: r.title,
                        published: r.published,
                    })
                    .collect::<Vec<_>>(),
            ))
            .into_response())
        }
        InnerRequest::UpdateArticle { id, title, content } => {
            match (title, content) {
                (Some(title), Some(content)) => {
                    sqlx::query!(
                        "UPDATE articles SET title = ?, content = ? WHERE id = ?",
                        title,
                        content,
                        id
                    )
                    .execute(&mut *conn)
                    .await
                    .into_diagnostic()?;
                }
                (None, Some(content)) => {
                    sqlx::query!("UPDATE articles SET content = ? WHERE id = ?", content, id)
                        .execute(&mut *conn)
                        .await
                        .into_diagnostic()?;
                }
                (Some(title), None) => {
                    sqlx::query!("UPDATE articles SET title = ? WHERE id = ?", title, id)
                        .execute(&mut *conn)
                        .await
                        .into_diagnostic()?;
                }

                (None, None) => (),
            }

            Ok(Json(Response::Ok).into_response())
        }
    }
}

async fn get_article(
    Path(url): Path<String>,
    State(state): State<BlogState>,
) -> Result<AxumResponse, TkError> {
    let mut conn = state.get_conn().await;
    let titles = sqlx::query!("SELECT id, title FROM articles")
        .fetch_all(&mut *conn)
        .await
        .into_diagnostic()?;

    match titles.iter().find_map(|r| {
        if to_url(&r.title) == url {
            Some(&r.id)
        } else {
            None
        }
    }) {
        Some(id) => {
            let article = sqlx::query_as!(Article, "SELECT * FROM articles WHERE id = ?", id)
                .fetch_one(&mut *conn)
                .await
                .unwrap();

            Ok(ArticleTemplate {
                config: state.config,
                article,
            }
            .into_response())
        }
        None => Ok(ErrorPage {
            config: state.config,
        }
        .into_response()),
    }
}

#[derive(Template)]
#[template(path = "index.html")]
struct IndexPage {
    config: ServerConfig,
    articles: Vec<Article>,
}

async fn index(State(state): State<BlogState>) -> Result<AxumResponse, TkError> {
    let mut conn = state.get_conn().await;
    let articles = sqlx::query_as!(Article, "SELECT * FROM articles ORDER BY published DESC")
        .fetch_all(&mut *conn)
        .await
        .into_diagnostic()?;

    Ok(IndexPage {
        config: state.config,
        articles,
    }
    .into_response())
}

#[derive(Template)]
#[template(path = "404.html")]
struct ErrorPage {
    config: ServerConfig,
}

async fn rss_feed(State(state): State<BlogState>) -> Result<AxumResponse, TkError> {
    let mut conn = state.get_conn().await;
    let articles = sqlx::query_as!(Article, "SELECT * FROM articles ORDER BY published DESC")
        .fetch_all(&mut *conn)
        .await
        .into_diagnostic()?;
    let channel = ChannelBuilder::default()
        .title(state.config.blog_name)
        .description(state.config.description)
        .items(articles.into_iter().map(Into::into).collect_vec())
        .build();

    Ok((
        [(header::CONTENT_TYPE, "application/rss+xml")],
        channel.to_string(),
    )
        .into_response())
}

pub async fn serve(config: ServerConfig) -> miette::Result<()> {
    let state = BlogState {
        pool: SqlitePool::connect("sqlite://articles.db")
            .await
            .into_diagnostic()?,
        config: config.clone(),
    };

    let error_cfg = config.clone();
    let router = Router::new()
        .nest_service(
            "/static",
            get_service(ServeDir::new("static").not_found_service(ServeFile::new("/404.html"))),
        )
        .route("/", get(index))
        .route("/article/:id", get(get_article))
        .route("/api", post(handle_api_request))
        .route("/rss", get(rss_feed))
        .fallback(get(|| async { ErrorPage { config: error_cfg } }))
        .with_state(state);

    let listener = TcpListener::bind(&config.addr).await.into_diagnostic()?;
    axum::serve(listener, router.into_make_service())
        .await
        .into_diagnostic()?;
    Ok(())
}

pub async fn create_secret(description: Option<String>) -> miette::Result<()> {
    let secret = Alphanumeric.sample_string(&mut thread_rng(), 64);

    let mut conn = SqliteConnectOptions::from_str("sqlite://articles.db")
        .into_diagnostic()?
        .connect()
        .await
        .into_diagnostic()?;

    sqlx::query!(
        "INSERT INTO secrets (secret, description) VALUES (?1, ?2)",
        secret,
        description
    )
    .execute(&mut conn)
    .await
    .into_diagnostic()?;

    println!("Your client secret is:");
    println!("{secret}");
    println!("Please note that you will *not* be able to see it again.");
    Ok(())
}

pub async fn list_secrets() -> miette::Result<()> {
    let mut conn = SqliteConnectOptions::from_str("sqlite://articles.db")
        .into_diagnostic()?
        .connect()
        .await
        .into_diagnostic()?;

    let secrets = sqlx::query!("SELECT id, description FROM secrets",)
        .fetch_all(&mut conn)
        .await
        .into_diagnostic()?;

    let mut table = Table::new();
    table.set_header(Row::from(vec!["ID", "Description"]));
    for row in secrets {
        table.add_row([
            &row.id.to_string(),
            &row.description.unwrap_or("-".to_string()),
        ]);
    }
    println!("{table}");

    Ok(())
}

pub async fn revoke_secret(id: i64) -> miette::Result<()> {
    let mut conn = SqliteConnectOptions::from_str("sqlite://articles.db")
        .into_diagnostic()?
        .connect()
        .await
        .into_diagnostic()?;

    sqlx::query!("DELETE FROM secrets where id = ?", id)
        .execute(&mut conn)
        .await
        .into_diagnostic()?;

    Ok(())
}

async fn is_secret_valid(secret: &str, conn: &mut SqliteConnection) -> miette::Result<bool> {
    Ok(
        sqlx::query!("SELECT id FROM secrets WHERE secret = ?", secret)
            .fetch_optional(conn)
            .await
            .into_diagnostic()?
            .is_some(),
    )
}
