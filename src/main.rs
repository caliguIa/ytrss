use clap::{Arg, Command, value_parser};
use futures::stream::{self, StreamExt};
use select::document::Document;
use select::predicate::Name;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use thiserror::Error;
use url::Url;

const MAX_CONCURRENT_REQUESTS: usize = 10;

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

    #[error("Invalid URL: {0}")]
    InvalidUrl(#[from] url::ParseError),
}

type Result<T> = std::result::Result<T, AppError>;

#[derive(Debug, Clone)]
pub struct YoutubeUrl {
    url: Url,
}
impl YoutubeUrl {
    pub fn new(url_str: &str) -> Result<Self> {
        let url = Url::parse(url_str)?;
        let host = url
            .host_str()
            .ok_or_else(|| AppError::UrlError("Missing host in URL".to_string()))?;

        if !host.contains("youtube.com") && !host.contains("youtu.be") {
            return Err(AppError::UrlError("Not a YouTube URL".to_string()));
        }

        Ok(Self { url })
    }

    pub fn as_str(&self) -> &str {
        self.url.as_str()
    }
}
impl AsRef<str> for YoutubeUrl {
    fn as_ref(&self) -> &str {
        self.url.as_str()
    }
}

pub struct YoutubeClient {
    client: reqwest::Client,
}
impl YoutubeClient {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_default();

        Self { client }
    }

    pub async fn fetch_html(&self, url: &YoutubeUrl) -> Result<String> {
        let response = self.client.get(url.as_str()).send().await?;

        if !response.status().is_success() {
            return Err(AppError::UrlError(format!(
                "Failed to fetch URL: HTTP status {}",
                response.status()
            )));
        }

        let html_content = response.text().await?;
        Ok(html_content)
    }
}
impl Default for YoutubeClient {
    fn default() -> Self {
        Self::new()
    }
}

pub struct HTMLParser;
impl HTMLParser {
    pub fn extract_feed_url(html_content: &str) -> Result<String> {
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

pub struct Output;
impl Output {
    pub fn print(url: &str) {
        println!("• {}", url);
    }

    pub fn generate_output_filename(path: &Path) -> PathBuf {
        let file_stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("output");

        let extension = path
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| format!(".{}", s))
            .unwrap_or_default();

        let parent_dir = path.parent().unwrap_or_else(|| Path::new(""));
        parent_dir.join(format!("{}_parsed{}", file_stem, extension))
    }

    pub fn write_urls(path: &Path, urls: &[String]) -> Result<()> {
        let output_path = Self::generate_output_filename(path);
        println!("Writing to: {}", output_path.display());
        use std::io::Write;

        let mut file = std::fs::File::create(&output_path)?;
        for url in urls {
            writeln!(file, "{}", url)?;
        }

        println!("URLs successfully written to file");
        Ok(())
    }
}

pub struct App {
    client: Arc<YoutubeClient>,
}
impl App {
    pub fn new() -> Self {
        Self {
            client: Arc::new(YoutubeClient::new()),
        }
    }

    pub async fn run(&self, url_str: &str) -> Result<String> {
        let youtube_url = YoutubeUrl::new(url_str)?;
        let html_content = self.client.fetch_html(&youtube_url).await?;
        HTMLParser::extract_feed_url(&html_content)
    }

    pub async fn run_file(&self, file_path: &Path) -> Result<Vec<(String, Result<String>)>> {
        let content = std::fs::read_to_string(file_path)?;

        let urls: Vec<_> = content
            .lines()
            .filter_map(|line| {
                let line = line.trim();
                if !line.is_empty() {
                    Some(line.to_string())
                } else {
                    None
                }
            })
            .collect();

        let client = Arc::clone(&self.client);

        // Process URLs concurrently with limited parallelism
        let results = stream::iter(urls)
            .map(|url| {
                let client = Arc::clone(&client);
                async move {
                    let result = async {
                        let youtube_url = YoutubeUrl::new(&url)?;
                        let html_content = client.fetch_html(&youtube_url).await?;
                        HTMLParser::extract_feed_url(&html_content)
                    }
                    .await;

                    (url, result)
                }
            })
            .buffer_unordered(MAX_CONCURRENT_REQUESTS)
            .collect::<Vec<_>>()
            .await;

        Ok(results)
    }
}
impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

fn cli() -> Command {
    Command::new("ytrss")
        .version(env!("CARGO_PKG_VERSION"))
        .subcommand_required(true)
        .arg_required_else_help(true)
        .about("Extract RSS feeds from YouTube URLs")
        .subcommand(
            Command::new("url")
                .about("Process a single YouTube URL")
                .arg(
                    Arg::new("yt_channel_url")
                        .help("YouTube channel URL to extract RSS feed from")
                        .value_name("YT_URL")
                        .required(true)
                        .index(1),
                )
                .arg_required_else_help(true),
        )
        .subcommand(
            Command::new("file")
                .about("Process a file containing YouTube URLs")
                .arg(
                    Arg::new("file_path")
                        .help("Input file with YouTube channel URLs (one per line)")
                        .value_name("FILE PATH")
                        .required(true)
                        .value_parser(value_parser!(std::path::PathBuf))
                        .index(1),
                ),
        )
}

#[tokio::main]
async fn main() -> Result<()> {
    let matches = cli().get_matches();
    let app = App::new();

    match matches.subcommand() {
        Some(("url", sub_matches)) => {
            let rss_url = app
                .run(
                    sub_matches
                        .get_one::<String>("yt_channel_url")
                        .expect("required"),
                )
                .await?;
            Output::print(&rss_url);
        }
        Some(("file", sub_matches)) => {
            let file_path = sub_matches
                .get_one::<std::path::PathBuf>("file_path")
                .expect("required");

            let results = app.run_file(file_path).await?;

            let successful_urls: Vec<_> = results
                .iter()
                .filter_map(|(_, result)| result.as_ref().ok().cloned())
                .collect();

            for (url, result) in &results {
                if let Err(e) = result {
                    eprintln!("Error processing {}: {}", url, e);
                }
            }

            if successful_urls.is_empty() {
                println!("No RSS feeds found.");
            } else {
                Output::write_urls(file_path, &successful_urls)?;
            }
        }
        _ => unreachable!(),
    }

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

    #[test]
    fn test_generate_output_filename_with_extension() {
        let path = PathBuf::from("/path/to/input.txt");
        let result = Output::generate_output_filename(&path);
        assert_eq!(result, PathBuf::from("/path/to/input_parsed.txt"));
    }

    #[test]
    fn test_generate_output_filename_without_extension() {
        let path = PathBuf::from("/path/to/inputfile");
        let result = Output::generate_output_filename(&path);
        assert_eq!(result, PathBuf::from("/path/to/inputfile_parsed"));
    }

    #[test]
    fn test_generate_output_filename_just_filename() {
        let path = PathBuf::from("data.csv");
        let result = Output::generate_output_filename(&path);
        assert_eq!(result, PathBuf::from("data_parsed.csv"));
    }

    #[test]
    fn test_generate_output_filename_with_multiple_dots() {
        let path = PathBuf::from("archive.tar.gz");
        let result = Output::generate_output_filename(&path);
        assert_eq!(result, PathBuf::from("archive.tar_parsed.gz"));
    }
}
