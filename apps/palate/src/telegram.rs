use color_eyre::eyre::{Result, WrapErr};
use reqwest::Url;
use teloxide::payloads::SendMessageSetters;
use teloxide::prelude::*;
use teloxide::types::{ChatId, LinkPreviewOptions, ParseMode};

use crate::config::TelegramConfig;

/// Something that can deliver a notification, so the job doesn't depend on Telegram directly.
///
/// The text must be valid Telegram `HTML` with any text escaped.
pub trait Notifier: Send + Sync + 'static {
    fn send_message(&self, text: &str) -> impl Future<Output = Result<()>> + Send;
}

#[derive(Clone)]
pub struct TelegramClient {
    bot: Bot,
    chat_id: ChatId,
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
        let api_url = Url::parse(&config.base_url).wrap_err("invalid Telegram base URL")?;
        let chat_id = config
            .chat_id
            .parse()
            .map(ChatId)
            .wrap_err("Telegram chat id must be a number")?;

        Ok(Self {
            bot: Bot::new(&*config.bot_token).set_api_url(api_url),
            chat_id,
        })
    }
}

impl Notifier for TelegramClient {
    #[tracing::instrument(skip(self, text))]
    async fn send_message(&self, text: &str) -> Result<()> {
        // teloxide strips the bot token from network errors, so they are safe to report
        self.bot
            .send_message(self.chat_id, text)
            .parse_mode(ParseMode::Html)
            // Links sit behind mTLS, so Telegram could never generate a preview
            .link_preview_options(LinkPreviewOptions {
                is_disabled: true,
                url: None,
                prefer_small_media: false,
                prefer_large_media: false,
                show_above_text: false,
            })
            .await
            .wrap_err("failed to send message to Telegram")?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use foundation_configuration::Secret;

    use crate::config::TelegramConfig;
    use crate::telegram::{Notifier, TelegramClient, conversation_ready_message};

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
