mod article;
mod client;
mod comment;
mod error;
mod request;
mod server;

use std::{collections::HashMap, net::SocketAddr};

use clap::{Args, Parser, Subcommand};
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
    /// Manage server-side secrets
    #[command(subcommand)]
    Secret(SecretOperation),
}

#[derive(Args)]
pub struct Publish {
    path: String,
    title: Option<String>,
}

#[derive(Subcommand)]
pub enum SecretOperation {
    /// Create a new secret, optionally with a description
    Create {
        #[arg(short, long)]
        description: Option<String>,
    },
    /// List the existing secrets by ID. Does not actually show the secrets.
    List,
    /// Revokes the secret with the given ID
    Revoke {
        #[arg(short, long)]
        id: i64,
    },
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
            server::serve(config.server.ok_or(miette!("no server config found"))?).await?
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
        Command::Secret(operation) => match operation {
            SecretOperation::Create { description } => server::create_secret(description).await?,
            SecretOperation::List => server::list_secrets().await?,
            SecretOperation::Revoke { id } => server::revoke_secret(id).await?,
        },
    }

    Ok(())
}
