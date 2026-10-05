use std::time::Duration;

use color_eyre::eyre::{Result, WrapErr, eyre};
use foundation_configuration::Secret;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::config::OpenRouterConfig;
use crate::persistence::Message;

#[derive(Clone)]
pub struct OpenRouterClient {
    http_client: Client,
    base_url: String,
    api_key: Secret<String>,
    model: String,
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage<'a>>,
}

#[derive(Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Deserialize)]
struct ResponseMessage {
    content: Option<String>,
}

impl OpenRouterClient {
    pub fn new(config: &OpenRouterConfig) -> Result<Self> {
        // Models can take a while to respond, particularly with long conversations
        let http_client = Client::builder()
            .timeout(Duration::from_secs(300))
            .build()?;

        Ok(Self {
            http_client,
            base_url: config.base_url.trim_end_matches('/').to_owned(),
            api_key: config.api_key.clone(),
            model: config.model.clone(),
        })
    }

    /// Sends the full conversation history and returns the content of the reply.
    #[tracing::instrument(skip(self, messages), fields(model = %self.model, messages = messages.len()))]
    pub async fn complete(&self, messages: &[Message]) -> Result<String> {
        let request = ChatRequest {
            model: &self.model,
            messages: messages
                .iter()
                .map(|m| ChatMessage {
                    role: m.role.as_str(),
                    content: &m.content,
                })
                .collect(),
        };

        let url = format!("{}/chat/completions", self.base_url);

        let response = self
            .http_client
            .post(url)
            .bearer_auth(&*self.api_key)
            .json(&request)
            .send()
            .await
            .wrap_err("failed to send request to OpenRouter")?;

        let status = response.status();

        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();

            return Err(eyre!("OpenRouter returned {status}: {body}"));
        }

        let response: ChatResponse = response
            .json()
            .await
            .wrap_err("failed to parse OpenRouter response")?;

        response
            .choices
            .into_iter()
            .next()
            .and_then(|choice| choice.message.content)
            .filter(|content| !content.trim().is_empty())
            .ok_or_else(|| eyre!("OpenRouter response contained no content"))
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use foundation_configuration::Secret;
    use mockito::{Matcher, Server};
    use serde_json::json;

    use crate::config::OpenRouterConfig;
    use crate::openrouter::OpenRouterClient;
    use crate::persistence::{Message, Role};

    fn client(base_url: String) -> OpenRouterClient {
        OpenRouterClient::new(&OpenRouterConfig {
            api_key: Secret::from("key".to_owned()),
            model: "test/model".to_owned(),
            base_url,
        })
        .unwrap()
    }

    fn message(role: Role, content: &str) -> Message {
        Message {
            role,
            content: content.to_owned(),
            created_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn sends_history_and_returns_reply() {
        let mut server = Server::new_async().await;

        let mock = server
            .mock("POST", "/chat/completions")
            .match_header("authorization", "Bearer key")
            .match_body(Matcher::Json(json!({
                "model": "test/model",
                "messages": [
                    {"role": "system", "content": "sys"},
                    {"role": "user", "content": "hi"},
                ],
            })))
            .with_header("content-type", "application/json")
            .with_body(
                json!({"choices": [{"message": {"role": "assistant", "content": "ideas"}}]})
                    .to_string(),
            )
            .create_async()
            .await;

        let content = client(server.url())
            .complete(&[message(Role::System, "sys"), message(Role::User, "hi")])
            .await
            .unwrap();

        assert_eq!(content, "ideas");
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn empty_choices_are_an_error() {
        let mut server = Server::new_async().await;

        server
            .mock("POST", "/chat/completions")
            .with_header("content-type", "application/json")
            .with_body(json!({"choices": []}).to_string())
            .create_async()
            .await;

        let result = client(server.url())
            .complete(&[message(Role::User, "hi")])
            .await;

        assert!(result.is_err());
    }
}
