use askama::Template;
use askama_axum::IntoResponse;
use axum::{
    extract::{Path, State},
    routing::{get, get_service, post},
    Json, Router,
};

use miette::IntoDiagnostic;

use sqlx::{pool::PoolConnection, Pool, Sqlite, SqlitePool};
use tokio::net::TcpListener;
use tower_http::services::{ServeDir, ServeFile};

use crate::{
    article::{Article, ArticleTemplate},
    request::{ArticleMetadata, Request, Response},
    ServerConfig,
};

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

    match request {
        Request::CreateArticle { title, content } => {
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
        Request::GetArticle { id } => {
            let article = sqlx::query_as!(Article, "SELECT * FROM articles WHERE id = ?", id)
                .fetch_one(&mut *conn)
                .await
                .unwrap();

            Json(serde_json::to_string(&article).unwrap()).into_response()
        }
        Request::YankArticle { id } => {
            sqlx::query!("DELETE FROM articles WHERE id = ?", id)
                .execute(&mut *conn)
                .await
                .unwrap();

            Json(Response::Ok).into_response()
        }
        Request::ListArticles => {
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
        Request::UpdateArticle { id, title, content } => {
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
