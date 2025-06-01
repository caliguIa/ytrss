use ansi_term::Colour;
use ansi_term::Style;
use clap::Parser;
use select::document::Document;
use select::predicate::Name;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Reqwest error: {0}")]
    ReqwestError(#[from] reqwest::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("RSS feed not found: {0}")]
    RssNotFound(String),

    #[error("URL error: {0}")]
    UrlError(String),
}

#[derive(Debug, Clone)]
pub struct YoutubeUrl {
    url: String,
}
impl YoutubeUrl {
    pub fn new(url: &str) -> Result<Self, AppError> {
        if !url.contains("youtube.com") && !url.contains("youtu.be") {
            return Err(AppError::UrlError("Not a YouTube URL".to_string()));
        }
        Ok(Self {
            url: url.to_string(),
        })
    }

    pub fn as_str(&self) -> &str {
        &self.url
    }
}

pub struct YoutubeClient;
impl YoutubeClient {
    pub async fn fetch_html(url: &YoutubeUrl) -> Result<String, AppError> {
        let response = reqwest::get(url.as_str()).await?;
        let html_content = response.text().await?;
        Ok(html_content)
    }
}

pub struct HTMLParser;
impl HTMLParser {
    /// Extract RSS feed URL from HTML content
    pub fn extract_feed_url(html_content: &str) -> Result<String, AppError> {
        let document = Document::from(html_content);

        document
            .find(Name("link"))
            .find(|node| {
                node.attr("title") == Some("RSS")
                    && node.attr("type") == Some("application/rss+xml")
            })
            .and_then(|node| node.attr("href"))
            .map(String::from)
            .ok_or_else(|| AppError::RssNotFound("RSS feed URL not found".to_string()))
    }
}

pub struct Formatter;
impl Formatter {
    pub fn print_rss_url(url: &str) {
        println!("RSS feed URL:");
        println!(
            "{} {}",
            Colour::Green.paint("•"),
            Style::new().bold().paint(url)
        );
    }
}

pub struct App;
impl App {
    pub async fn run(url: &str) -> Result<String, AppError> {
        let youtube_url = YoutubeUrl::new(url)?;
        let html_content = YoutubeClient::fetch_html(&youtube_url).await?;
        HTMLParser::extract_feed_url(&html_content)
    }
}

#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Args {
    url: String,
}

#[tokio::main]
async fn main() -> Result<(), AppError> {
    let args = Args::parse();
    let rss_url = App::run(&args.url).await?;
    Formatter::print_rss_url(&rss_url);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_youtube_url_valid() {
        let result = YoutubeUrl::new("https://www.youtube.com/channel/1234");
        assert!(result.is_ok());
    }

    #[test]
    fn test_youtube_url_invalid() {
        let result = YoutubeUrl::new("https://example.com");
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_rss_feed_url() {
        let html = r#"
        <html>
            <head>
                <link rel="alternate" type="application/rss+xml" title="RSS" href="https://www.youtube.com/feeds/videos.xml?channel_id=1234">
            </head>
        </html>
        "#;

        let result = HTMLParser::extract_feed_url(html);
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            "https://www.youtube.com/feeds/videos.xml?channel_id=1234"
        );
    }

    #[test]
    fn test_extract_rss_feed_url_not_found() {
        let html = "<html><head></head></html>";
        let result = HTMLParser::extract_feed_url(html);
        assert!(result.is_err());
    }
}
