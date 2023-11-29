use std::io::Write;

use comfy_table::{Row, Table};
use miette::IntoDiagnostic;
use reqwest::Client;

use crate::{
    request::{Request, Response},
    ClientConfig, Publish,
};

pub async fn publish(article: Publish, conf: ClientConfig) -> miette::Result<()> {
    let content = tokio::fs::read_to_string(article.path)
        .await
        .into_diagnostic()?;

    let title = match article.title {
        Some(t) => t,
        None => {
            print!("Please enter a title for the post: ");
            std::io::stdout().flush().into_diagnostic()?;
            let mut buf = String::new();
            std::io::stdin().read_line(&mut buf).into_diagnostic()?;
            buf.trim().to_owned()
        }
    };

    let request = Request::CreateArticle { title, content };
    let client = Client::new();
    client
        .post(format!("{}/api", conf.addr))
        .json(&request)
        .send()
        .await
        .into_diagnostic()?;

    Ok(())
}

pub async fn list(conf: ClientConfig) -> miette::Result<()> {
    let resp = Client::new()
        .post(format!("{}/api", conf.addr))
        .json(&Request::ListArticles)
        .send()
        .await
        .into_diagnostic()?;
    let data: Response = resp.json().await.into_diagnostic()?;

    match data {
        Response::ArticleMetadata(data) => {
            let mut table = Table::new();
            table.set_header(Row::from(vec!["ID", "Title", "Publication Date"]));
            for row in data {
                table.add_row(Row::from(&[
                    &row.id,
                    &row.title,
                    &row.published.to_string(),
                ]));
            }
            println!("{table}");
        }
        Response::Error(e) => println!("An error occured: {e}"),
        _ => unreachable!(),
    }

    Ok(())
}

pub async fn yank(conf: ClientConfig, id: String) -> miette::Result<()> {
    let resp = Client::new()
        .post(format!("{}/api", conf.addr))
        .json(&Request::YankArticle { id })
        .send()
        .await
        .into_diagnostic()?;
    let data: Response = resp.json().await.into_diagnostic()?;
    if let Response::Error(e) = data {
        println!("An error occured: {e}");
    }

    Ok(())
}

pub async fn update(
    conf: ClientConfig,
    id: String,
    title: Option<String>,
    path: Option<String>,
) -> miette::Result<()> {
    let content = if let Some(path) = path {
        Some(tokio::fs::read_to_string(path).await.into_diagnostic()?)
    } else {
        None
    };

    let resp = Client::new()
        .post(format!("{}/api", conf.addr))
        .json(&Request::UpdateArticle { id, title, content })
        .send()
        .await
        .into_diagnostic()?;
    let data: Response = resp.json().await.into_diagnostic()?;
    if let Response::Error(e) = data {
        println!("An error occured: {e}");
    }

    Ok(())
}
