use std::time::Duration;

use color_eyre::eyre::{Result, WrapErr};
use scraper::{Html, Node};

use super::{DomainFetcher, RetrievedContent};

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
    async fn fetch_url(&self, url: &str) -> Result<Option<RetrievedContent>> {
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

        let truncated = truncate_content_if_required(&mut body, MAX_BODY_BYTES);

        let html = String::from_utf8_lossy(&body);
        let text = extract_text(&html, self.max_chars);

        Ok(Some(RetrievedContent {
            raw: body,
            text,
            truncated,
        }))
    }
}

#[async_trait::async_trait]
impl DomainFetcher for ReqwestFetcher {
    async fn fetch(&self, domain: &str) -> Result<Option<RetrievedContent>> {
        let https = format!("https://{domain}/");

        match self.fetch_url(&https).await {
            Ok(content) => Ok(content),
            Err(https_error) => {
                tracing::warn!(error = %format_args!("{https_error:#}"), %domain, "https fetch failed, trying http");

                self.fetch_url(&format!("http://{domain}/"))
                    .await
                    .wrap_err("failed to fetch over both https and http")
            }
        }
    }
}

/// Cut `body` down to the download limit, returning whether anything was cut or the limit was
/// reached (so the document may be incomplete).
fn truncate_content_if_required(body: &mut Vec<u8>, max_bytes: usize) -> bool {
    let truncated = body.len() > max_bytes;
    body.truncate(max_bytes);

    truncated
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

    // Postgres can't store NUL characters in text.
    text.chars()
        .filter(|c| *c != '\0')
        .take(max_chars)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{extract_text, truncate_content_if_required};

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

    const MAX_BODY_BYTES_TEST: usize = 1024;

    #[test]
    fn bodies_over_the_limit_are_cut_and_marked_truncated() {
        let mut body = vec![b'a'; MAX_BODY_BYTES_TEST + 10];

        assert!(truncate_content_if_required(&mut body, MAX_BODY_BYTES_TEST));
        assert_eq!(body.len(), MAX_BODY_BYTES_TEST);
    }

    #[test]
    fn bodies_under_the_limit_are_untouched() {
        let mut body = b"<p>hi</p>".to_vec();

        assert!(!truncate_content_if_required(
            &mut body,
            MAX_BODY_BYTES_TEST
        ));
        assert_eq!(body, b"<p>hi</p>");
    }

    #[test]
    fn nul_characters_are_removed_from_the_text() {
        assert_eq!(extract_text("<p>ab\0cd</p>", 100), "abcd");
    }
}
