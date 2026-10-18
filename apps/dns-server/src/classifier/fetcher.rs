use std::time::Duration;

use color_eyre::eyre::{Result, WrapErr};
use scraper::{Html, Node};

use super::DomainFetcher;

/// The most bytes of a response body that will be downloaded.
const MAX_BODY_BYTES: usize = 1024 * 1024;

const SKIPPED_ELEMENTS: [&str; 7] = [
    "script", "style", "noscript", "template", "svg", "canvas", "iframe",
];

/// Fetches the root document of a domain over HTTP(S) and reduces it to plain text.
#[derive(Clone)]
pub struct ReqwestFetcher {
    client: reqwest::Client,
    max_chars: usize,
}

impl ReqwestFetcher {
    pub fn new(timeout: Duration, max_chars: usize) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .user_agent("dns-server-classifier/0.1")
            .build()?;

        Ok(Self { client, max_chars })
    }

    #[tracing::instrument(skip(self))]
    async fn fetch_url(&self, url: &str) -> Result<Option<String>> {
        let mut response = self.client.get(url).send().await?;

        tracing::info!("fetching content from a remote server");

        if !response.status().is_success() {
            tracing::warn!(status = %response.status(), "got an unsuccessful response from the server");
            return Ok(None);
        }

        let is_text = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_none_or(|value| value.starts_with("text/html") || value.starts_with("text/plain"));

        if !is_text {
            tracing::warn!("received no content from the server");
            return Ok(None);
        }

        let mut body = Vec::new();

        while let Some(chunk) = response.chunk().await? {
            body.extend_from_slice(&chunk);

            if body.len() >= MAX_BODY_BYTES {
                break;
            }
        }

        let html = String::from_utf8_lossy(&body);
        let text = extract_text(&html, self.max_chars);

        Ok(Some(text).filter(|text| !text.is_empty()))
    }
}

#[async_trait::async_trait]
impl DomainFetcher for ReqwestFetcher {
    async fn fetch(&self, domain: &str) -> Result<Option<String>> {
        let https = format!("https://{domain}/");

        match self.fetch_url(&https).await {
            Ok(content) => Ok(content),
            Err(https_error) => {
                tracing::warn!(error = ?https_error, %domain, "https fetch failed, trying http");

                self.fetch_url(&format!("http://{domain}/"))
                    .await
                    .wrap_err("failed to fetch over both https and http")
            }
        }
    }
}

/// Extract the visible text from `html`, ignoring scripts and styles, collapsing whitespace and
/// truncating to at most `max_chars` characters.
fn extract_text(html: &str, max_chars: usize) -> String {
    let document = Html::parse_document(html);
    let mut text = String::new();

    'nodes: for node in document.tree.nodes() {
        let Node::Text(content) = node.value() else {
            continue;
        };

        let skipped = node.ancestors().any(|ancestor| {
            ancestor
                .value()
                .as_element()
                .is_some_and(|el| SKIPPED_ELEMENTS.contains(&el.name()))
        });

        if skipped {
            continue;
        }

        for word in content.split_whitespace() {
            if !text.is_empty() {
                text.push(' ');
            }

            text.push_str(word);

            if text.chars().count() >= max_chars {
                break 'nodes;
            }
        }
    }

    text.chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use super::extract_text;

    #[test]
    fn strips_scripts_styles_and_tags() {
        let html = r#"
            <html>
              <head><title>My   Site</title><style>body { color: red }</style></head>
              <body>
                <script>var x = "secret";</script>
                <h1>Hello</h1>
                <p>some <b>bold</b>
                   text</p>
                <noscript>enable js</noscript>
              </body>
            </html>
        "#;

        assert_eq!(extract_text(html, 1000), "My Site Hello some bold text");
    }

    #[test]
    fn truncates_to_the_character_limit() {
        let html = "<p>abcdefghij klmnop</p>";

        assert_eq!(extract_text(html, 5), "abcde");
    }

    #[test]
    fn truncation_respects_multibyte_characters() {
        let html = "<p>ééééé</p>";

        assert_eq!(extract_text(html, 3), "ééé");
    }

    #[test]
    fn empty_documents_produce_no_text() {
        assert_eq!(extract_text("<html><script>x</script></html>", 100), "");
    }
}
