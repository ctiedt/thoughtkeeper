mod article;
mod client;
mod request;
mod serve;

use std::{collections::HashMap, net::SocketAddr};

use clap::{Args, Parser};
use figment::{
    providers::{Format, Toml},
    Figment,
};
use miette::{miette, IntoDiagnostic};
use serde::Deserialize;

#[derive(Parser)]
#[command(author, version, about)]
pub enum Command {
    /// Serve the blog on the configured address
    Serve,
    /// Publish an article to a blog
    Publish(Publish),
    /// List all published articles
    List,
    /// Yank (delete) the article with the given ID
    Yank { id: String },
    /// Update the title or content of an existing article
    Update {
        /// The article to update
        id: String,
        #[arg(short, long)]
        /// The new title
        title: Option<String>,
        #[arg(short, long)]
        /// The path of the updated content
        path: Option<String>,
    },
}

#[derive(Args)]
pub struct Publish {
    path: String,
    title: Option<String>,
}

#[derive(Deserialize)]
pub struct Config {
    server: Option<ServerConfig>,
    client: Option<ClientConfig>,
}

#[derive(Deserialize, Clone)]
pub struct ServerConfig {
    blog_name: String,
    author: String,
    description: String,
    footer_links: HashMap<String, String>,
    addr: SocketAddr,
    domain: Option<String>,
}

#[derive(Deserialize)]
pub struct ClientConfig {
    addr: String,
    secret: String,
}

#[tokio::main]
async fn main() -> miette::Result<()> {
    let command = Command::parse();

    let config: Config = Figment::new()
        .merge(Toml::file("blog.toml"))
        .extract()
        .into_diagnostic()?;

    match command {
        Command::Serve => {
            serve::serve(config.server.ok_or(miette!("no server config found"))?).await?
        }
        Command::Publish(article) => {
            client::publish(
                article,
                config.client.ok_or(miette!("no client config found"))?,
            )
            .await?
        }
        Command::List => {
            client::list(config.client.ok_or(miette!("no client config found"))?).await?
        }
        Command::Yank { id } => {
            client::yank(config.client.ok_or(miette!("no client config found"))?, id).await?
        }
        Command::Update { id, title, path } => {
            client::update(
                config.client.ok_or(miette!("no client config found"))?,
                id,
                title,
                path,
            )
            .await?
        }
    }

    Ok(())
}
