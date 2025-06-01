use clap::Parser;
use select::document::Document;
use select::predicate::Name;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Reqwest error: {0}")]
    ReqError(#[from] reqwest::Error),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("{0}")]
    RssNotFound(String),
}
impl AppError {
    fn rss_not_found() -> Self {
        AppError::RssNotFound("RSS feed URL not found".to_string())
    }
}

#[derive(Parser)]
struct Args {
    url: String,
}

#[tokio::main]
async fn main() -> Result<(), AppError> {
    let args = Args::parse();
    let rss_url = get_rss_feed_url(args.url).await?;

    println!("RSS feed URL:");
    println!("{}", rss_url);

    Ok(())
}

async fn get_rss_feed_url(url: String) -> Result<String, AppError> {
    let res = reqwest::get(url).await?.text().await?;

    let document = Document::from(res.as_str());
    let feed_link_node = document.find(Name("link")).find(|node| {
        node.attr("title") == Some("RSS") && node.attr("type") == Some("application/rss+xml")
    });

    match feed_link_node {
        Some(n) => match n.attr("href") {
            Some(href) => Ok(href.to_string()),
            None => Err(AppError::rss_not_found()),
        },
        None => Err(AppError::rss_not_found()),
    }
}
