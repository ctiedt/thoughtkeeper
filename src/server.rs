use askama::Template;
use askama_axum::IntoResponse;
use axum::{
    extract::{Path, State},
    routing::{get, get_service, post},
    Json, Router,
};

use miette::IntoDiagnostic;

use sqlx::{
    pool::PoolConnection, sqlite::SqliteConnectOptions, ConnectOptions, Pool, Sqlite,
    SqliteConnection, SqlitePool,
};
use tokio::net::TcpListener;
use tower_http::services::{ServeDir, ServeFile};

use crate::{
    article::{Article, ArticleTemplate},
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
) -> impl IntoResponse {
    let mut conn = state.get_conn().await;

    if !is_secret_valid(&request.secret, &mut conn).await.unwrap() {
        return Json(Response::Error("Invalid secret".to_string())).into_response();
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
            .unwrap();

            Json(Response::ArticleId(article.id)).into_response()
        }
        InnerRequest::GetArticle { id } => {
            let article = sqlx::query_as!(Article, "SELECT * FROM articles WHERE id = ?", id)
                .fetch_one(&mut *conn)
                .await
                .unwrap();

            Json(serde_json::to_string(&article).unwrap()).into_response()
        }
        InnerRequest::YankArticle { id } => {
            sqlx::query!("DELETE FROM articles WHERE id = ?", id)
                .execute(&mut *conn)
                .await
                .unwrap();

            Json(Response::Ok).into_response()
        }
        InnerRequest::ListArticles => {
            let articles = sqlx::query!("SELECT id, title, published FROM articles")
                .fetch_all(&mut *conn)
                .await
                .unwrap();

            Json(Response::ArticleMetadata(
                articles
                    .into_iter()
                    .map(|r| ArticleMetadata {
                        id: r.id,
                        title: r.title,
                        published: r.published,
                    })
                    .collect::<Vec<_>>(),
            ))
            .into_response()
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
                    .unwrap();
                }
                (None, Some(content)) => {
                    sqlx::query!("UPDATE articles SET content = ? WHERE id = ?", content, id)
                        .execute(&mut *conn)
                        .await
                        .unwrap();
                }
                (Some(title), None) => {
                    sqlx::query!("UPDATE articles SET title = ? WHERE id = ?", title, id)
                        .execute(&mut *conn)
                        .await
                        .unwrap();
                }

                (None, None) => (),
            }

            Json(Response::Ok).into_response()
        }
    }
}

async fn get_article(Path(id): Path<String>, State(state): State<BlogState>) -> impl IntoResponse {
    let mut conn = state.get_conn().await;
    match sqlx::query_as!(Article, "SELECT * FROM articles WHERE id = ?", id)
        .fetch_one(&mut *conn)
        .await
    {
        Ok(article) => ArticleTemplate {
            config: state.config,
            article,
        }
        .into_response(),
        Err(_) => ErrorPage {
            config: state.config,
        }
        .into_response(),
    }
}

#[derive(Template)]
#[template(path = "index.html")]
struct IndexPage {
    config: ServerConfig,
    articles: Vec<Article>,
}

async fn index(State(state): State<BlogState>) -> impl IntoResponse {
    let mut conn = state.get_conn().await;
    let articles = sqlx::query_as!(Article, "SELECT * FROM articles ORDER BY published DESC")
        .fetch_all(&mut *conn)
        .await
        .unwrap();

    IndexPage {
        config: state.config,
        articles,
    }
}

#[derive(Template)]
#[template(path = "404.html")]
struct ErrorPage {
    config: ServerConfig,
}

pub async fn serve(config: ServerConfig) -> miette::Result<()> {
    let state = BlogState {
        pool: SqlitePool::connect("sqlite://articles.db").await.unwrap(),
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
