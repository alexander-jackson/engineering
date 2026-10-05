use std::time::Duration;

use color_eyre::eyre::{Result, WrapErr, eyre};
use openrouter_rs::api::chat::{ChatCompletionRequest, Message as ChatMessage};
use openrouter_rs::types::Role as ChatRole;

use crate::config::OpenRouterConfig;
use crate::persistence::{Message, Role};

/// Models can take a while to respond, particularly with long conversations
const REQUEST_TIMEOUT: Duration = Duration::from_secs(300);

/// Something that can continue a conversation, so callers don't depend on OpenRouter directly.
pub trait ChatModel: Clone + Send + Sync + 'static {
    /// Sends the full conversation history and returns the content of the reply.
    fn complete(&self, messages: &[Message]) -> impl Future<Output = Result<String>> + Send;
}

#[derive(Clone)]
pub struct OpenRouterClient {
    client: openrouter_rs::OpenRouterClient,
    model: String,
}

impl From<Role> for ChatRole {
    fn from(role: Role) -> Self {
        match role {
            Role::System => Self::System,
            Role::User => Self::User,
            Role::Assistant => Self::Assistant,
        }
    }
}

impl OpenRouterClient {
    pub fn new(config: &OpenRouterConfig) -> Result<Self> {
        let client = openrouter_rs::OpenRouterClient::builder()
            .base_url(config.base_url.trim_end_matches('/'))
            .api_key(&*config.api_key)
            .build()
            .wrap_err("failed to build OpenRouter client")?;

        Ok(Self {
            client,
            model: config.model.clone(),
        })
    }
}

impl ChatModel for OpenRouterClient {
    #[tracing::instrument(skip(self, messages), fields(model = %self.model, messages = messages.len()))]
    async fn complete(&self, messages: &[Message]) -> Result<String> {
        let messages = messages
            .iter()
            .map(|m| ChatMessage::new(m.role.into(), m.content.as_str()))
            .collect();

        let request = ChatCompletionRequest::builder()
            .model(self.model.as_str())
            .messages(messages)
            .build()
            .wrap_err("failed to build OpenRouter request")?;

        let response =
            tokio::time::timeout(REQUEST_TIMEOUT, self.client.send_chat_completion(&request))
                .await
                .wrap_err("timed out waiting for OpenRouter")?
                .wrap_err("failed to get a completion from OpenRouter")?;

        response
            .choices
            .first()
            .and_then(|choice| choice.content())
            .filter(|content| !content.trim().is_empty())
            .map(str::to_owned)
            .ok_or_else(|| eyre!("OpenRouter response contained no content"))
    }
}

#[cfg(test)]
pub mod testing {
    use std::sync::{Arc, Mutex};

    use color_eyre::eyre::{Result, eyre};

    use crate::openrouter::ChatModel;
    use crate::persistence::{Message, Role};

    /// The history sent with a single call.
    type History = Vec<(Role, String)>;

    /// A model that replies with a fixed answer and records what it was asked.
    #[derive(Clone)]
    pub struct ScriptedModel {
        reply: Option<String>,
        requests: Arc<Mutex<Vec<History>>>,
    }

    impl ScriptedModel {
        pub fn replying(reply: &str) -> Self {
            Self {
                reply: Some(reply.to_owned()),
                requests: Default::default(),
            }
        }

        pub fn failing() -> Self {
            Self {
                reply: None,
                requests: Default::default(),
            }
        }

        /// The history sent with each call, in order.
        pub fn requests(&self) -> Vec<History> {
            self.requests.lock().unwrap().clone()
        }
    }

    impl ChatModel for ScriptedModel {
        async fn complete(&self, messages: &[Message]) -> Result<String> {
            let history = messages
                .iter()
                .map(|m| (m.role, m.content.clone()))
                .collect();
            self.requests.lock().unwrap().push(history);

            self.reply.clone().ok_or_else(|| eyre!("model unavailable"))
        }
    }
}

#[cfg(test)]
mod tests {
    use foundation_configuration::Secret;

    use crate::config::OpenRouterConfig;
    use crate::openrouter::{ChatModel, OpenRouterClient};

    #[tokio::test]
    async fn errors_do_not_leak_the_api_key() {
        let client = OpenRouterClient::new(&OpenRouterConfig {
            api_key: Secret::from("super-secret".to_owned()),
            model: "test/model".to_owned(),
            base_url: "http://127.0.0.1:1".to_owned(),
        })
        .unwrap();

        let error = client.complete(&[]).await.unwrap_err();

        assert!(!format!("{error:?}").contains("super-secret"));
    }
}
