use std::time::Duration;

use color_eyre::eyre::{Result, WrapErr, eyre};
use foundation_configuration::Secret;
use reqwest::Client;
use serde::Serialize;

use crate::config::TelegramConfig;

#[derive(Clone)]
pub struct TelegramClient {
    http_client: Client,
    base_url: String,
    bot_token: Secret<String>,
    chat_id: String,
}

#[derive(Serialize)]
struct SendMessageRequest<'a> {
    chat_id: &'a str,
    text: &'a str,
    parse_mode: &'static str,
    disable_web_page_preview: bool,
}

/// Escapes text for use in a message sent with the `HTML` parse mode.
fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Builds the message announcing a new conversation, as `HTML` for [`TelegramClient::send_message`].
///
/// The link is an explicit anchor, as Telegram only auto-links URLs on recognisable domains.
pub fn conversation_ready_message(url: &str) -> String {
    format!(
        "🍽️ <b>Your recipe ideas for this week are ready</b>\n\n<a href=\"{}\">Open the conversation</a>",
        escape_html(url)
    )
}

impl TelegramClient {
    pub fn new(config: &TelegramConfig) -> Result<Self> {
        let http_client = Client::builder().timeout(Duration::from_secs(30)).build()?;

        Ok(Self {
            http_client,
            base_url: config.base_url.trim_end_matches('/').to_owned(),
            bot_token: config.bot_token.clone(),
            chat_id: config.chat_id.clone(),
        })
    }

    /// Sends a message, which must be valid Telegram `HTML` with any text escaped.
    #[tracing::instrument(skip(self, text))]
    pub async fn send_message(&self, text: &str) -> Result<()> {
        // The bot token is part of the URL, so make sure it never ends up in an error
        let url = format!("{}/bot{}/sendMessage", self.base_url, *self.bot_token);
        let request = SendMessageRequest {
            chat_id: &self.chat_id,
            text,
            parse_mode: "HTML",
            // Links sit behind mTLS, so Telegram could never generate a preview
            disable_web_page_preview: true,
        };

        let response = self
            .http_client
            .post(url)
            .json(&request)
            .send()
            .await
            .map_err(reqwest::Error::without_url)
            .wrap_err("failed to send request to Telegram")?;

        let status = response.status();

        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();

            return Err(eyre!("Telegram returned {status}: {body}"));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use foundation_configuration::Secret;
    use mockito::{Matcher, Server};
    use serde_json::json;

    use crate::config::TelegramConfig;
    use crate::telegram::{TelegramClient, conversation_ready_message};

    #[tokio::test]
    async fn sends_message_to_configured_chat() {
        let mut server = Server::new_async().await;

        // The bot token is part of the path
        let mock = server
            .mock("POST", "/bottoken/sendMessage")
            .match_body(Matcher::Json(json!({
                "chat_id": "-100",
                "text": "hello",
                "parse_mode": "HTML",
                "disable_web_page_preview": true,
            })))
            .with_header("content-type", "application/json")
            .with_body(json!({"ok": true}).to_string())
            .create_async()
            .await;

        let client = TelegramClient::new(&TelegramConfig {
            bot_token: Secret::from("token".to_owned()),
            chat_id: "-100".to_owned(),
            base_url: server.url(),
        })
        .unwrap();

        client.send_message("hello").await.unwrap();

        mock.assert_async().await;
    }
    #[test]
    fn conversation_message_links_with_an_anchor() {
        let message = conversation_ready_message("https://palate.test/conversations/abc?a=1&b=2");

        assert_eq!(
            message,
            "🍽️ <b>Your recipe ideas for this week are ready</b>\n\n\
             <a href=\"https://palate.test/conversations/abc?a=1&amp;b=2\">Open the conversation</a>"
        );
    }

    #[tokio::test]
    async fn errors_do_not_leak_the_bot_token() {
        let client = TelegramClient::new(&TelegramConfig {
            bot_token: Secret::from("super-secret".to_owned()),
            chat_id: "1".to_owned(),
            base_url: "http://127.0.0.1:1".to_owned(),
        })
        .unwrap();

        let error = client.send_message("hello").await.unwrap_err();

        assert!(!format!("{error:?}").contains("super-secret"));
    }
}
