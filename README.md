# ytrss

A CLI tool to extract RSS feed URLs from YouTube channel URLs.

## Installation

```bash
cargo install ytrss
```

## Usage

### Single URL

Extract RSS feed URL from a single YouTube channel:

```bash
ytrss url "https://www.youtube.com/channel/xxx"
```

### File Input

Process multiple YouTube URLs from a file (one URL per line):

```bash
ytrss file channels.txt
```

The tool will create an output file with `_parsed` suffix containing the RSS feed URLs.

## Examples

```bash
# Single channel
ytrss url "https://www.youtube.com/@example"

# Multiple channels from file
echo "https://www.youtube.com/channel/UC1234" > channels.txt
echo "https://www.youtube.com/@example" >> channels.txt
ytrss file channels.txt
```

## Features

- Supports both youtube.com and youtu.be URLs
- Concurrent processing for file input (up to 10 requests)
- Automatic output file generation for batch processing
