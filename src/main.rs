use clap::Parser;
use clap::error::Result;
use select::document::Document;
use select::predicate::Name;
use std::path::Path;
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

pub struct Output;
impl Output {
    pub fn print(url: &str) {
        println!("• {}", url);
    }
    fn generate_output_filename(path: &Path) -> String {
        path.file_name()
            .and_then(|f| f.to_str())
            .map(|name| {
                if let Some(pos) = name.rfind('.') {
                    let (base, ext) = name.split_at(pos);
                    format!("{}_parsed{}", base, ext)
                } else {
                    format!("{}_parsed", name)
                }
            })
            .unwrap_or_else(|| "output_parsed".to_string())
    }
    fn write_urls<W: std::io::Write>(writer: &mut W, urls: &[String]) -> std::io::Result<()> {
        for url in urls {
            writeln!(writer, "{}", url)?;
        }
        Ok(())
    }
    pub fn file(path: &Path, urls: Vec<String>) {
        let new_file_name = Self::generate_output_filename(path);
        let parent_dir = path.parent().unwrap_or_else(|| Path::new(""));
        let new_path = parent_dir.join(new_file_name);

        println!("Writing to: {}", new_path.display());
        match std::fs::File::create(new_path) {
            Ok(mut file) => {
                if let Err(e) = Self::write_urls(&mut file, &urls) {
                    eprintln!("Error writing to file: {}", e);
                } else {
                    println!("URLS successfully parsed");
                }
            }
            Err(e) => eprintln!("Error creating file: {}", e),
        }
    }
}

pub struct App;
impl App {
    pub async fn run(url: &str) -> Result<String, AppError> {
        let youtube_url = YoutubeUrl::new(url)?;
        let html_content = YoutubeClient::fetch_html(&youtube_url).await?;
        HTMLParser::extract_feed_url(&html_content)
    }
    pub async fn run_file(file_path: &Path) -> Result<Vec<String>, AppError> {
        let content = std::fs::read_to_string(file_path)?;
        let mut urls = Vec::new();
        for line in content.lines() {
            let line = line.trim();
            if !line.is_empty() {
                match Self::run(line).await {
                    Ok(rss_url) => urls.push(rss_url),
                    Err(e) => eprintln!("Error processing URL {}: {}", line, e),
                }
            }
        }
        Ok(urls)
    }
}

#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Args {
    #[arg(short = 'u', long = "url", conflicts_with = "input_path")]
    url: Option<String>,
    #[arg(short = 'i', long = "input", conflicts_with = "url")]
    input_path: Option<std::path::PathBuf>,
    #[arg(short = 'o', long = "output", conflicts_with = "url")]
    url_positional: Option<String>,
}
impl Args {
    fn get_url(&self) -> Option<String> {
        self.url.clone().or(self.url_positional.clone())
    }
}

#[tokio::main]
async fn main() -> Result<(), AppError> {
    let args = Args::parse();

    if let Some(url) = args.get_url() {
        let rss_url = App::run(&url).await?;
        Output::print(&rss_url);
    } else if let Some(file_path) = args.input_path.as_ref() {
        let rss_urls = App::run_file(file_path).await?;
        if rss_urls.is_empty() {
            println!("No RSS feeds found.");
        } else {
            Output::file(file_path, rss_urls);
        }
    } else {
        eprintln!("Error: Please provide either a YouTube URL or an input file path.");
        std::process::exit(1);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

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
        assert_eq!(result, "input_parsed.txt");
    }

    #[test]
    fn test_generate_output_filename_without_extension() {
        let path = PathBuf::from("/path/to/inputfile");
        let result = Output::generate_output_filename(&path);
        assert_eq!(result, "inputfile_parsed");
    }

    #[test]
    fn test_generate_output_filename_just_filename() {
        let path = PathBuf::from("data.csv");
        let result = Output::generate_output_filename(&path);
        assert_eq!(result, "data_parsed.csv");
    }

    #[test]
    fn test_generate_output_filename_with_multiple_dots() {
        let path = PathBuf::from("archive.tar.gz");
        let result = Output::generate_output_filename(&path);
        assert_eq!(result, "archive.tar_parsed.gz");
    }

    #[test]
    fn test_write_urls_to_buffer() {
        let urls = vec![
            "https://www.youtube.com/feeds/videos.xml?channel_id=1234".to_string(),
            "https://www.youtube.com/feeds/videos.xml?channel_id=5678".to_string(),
        ];

        let mut buffer = Vec::new();
        let result = Output::write_urls(&mut buffer, &urls);

        assert!(result.is_ok());

        let content = String::from_utf8(buffer).unwrap();
        let lines: Vec<&str> = content.lines().collect();

        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], urls[0]);
        assert_eq!(lines[1], urls[1]);
    }

    #[test]
    fn test_write_empty_urls_to_buffer() {
        let urls: Vec<String> = vec![];

        let mut buffer = Vec::new();
        let result = Output::write_urls(&mut buffer, &urls);

        assert!(result.is_ok());
        assert_eq!(buffer.len(), 0, "Buffer should be empty");
    }
}
