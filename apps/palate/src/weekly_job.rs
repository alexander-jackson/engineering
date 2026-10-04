use chrono::Utc;
use color_eyre::eyre::Result;
use foundation_recurring_job::{Job, Schedule};
use sqlx::PgPool;

use crate::config::ScheduleConfig;
use crate::openrouter::OpenRouterClient;
use crate::persistence::Role;
use crate::prompt::{SYSTEM_PROMPT, WEEKLY_PROMPT};
use crate::telegram::{TelegramClient, conversation_ready_message};
use crate::uid::ConversationUid;

pub struct WeeklyRecipes {
    pool: PgPool,
    openrouter: OpenRouterClient,
    telegram: TelegramClient,
    base_url: String,
    schedule: ScheduleConfig,
}

impl WeeklyRecipes {
    pub fn new(
        pool: PgPool,
        openrouter: OpenRouterClient,
        telegram: TelegramClient,
        base_url: &str,
        schedule: ScheduleConfig,
    ) -> Self {
        Self {
            pool,
            openrouter,
            telegram,
            base_url: base_url.trim_end_matches('/').to_owned(),
            schedule,
        }
    }
}

impl Job for WeeklyRecipes {
    const NAME: &'static str = "Weekly Recipes";

    fn schedule(&self) -> Schedule {
        let ScheduleConfig {
            weekday,
            hour,
            minute,
        } = self.schedule;

        Schedule::weekly(weekday, hour, minute)
    }

    async fn run(&self) -> Result<()> {
        let conversation_uid = ConversationUid::new();

        // Persist the prompts first, so a failed model call leaves a record of what was sent
        crate::persistence::create_conversation(
            &self.pool,
            conversation_uid,
            &[(Role::System, SYSTEM_PROMPT), (Role::User, WEEKLY_PROMPT)],
            Utc::now(),
        )
        .await?;

        crate::conversation::generate_reply(&self.pool, &self.openrouter, conversation_uid).await?;

        tracing::info!(%conversation_uid, "generated weekly recipe ideas");

        let url = format!("{}/conversations/{conversation_uid}", self.base_url);

        self.telegram
            .send_message(&conversation_ready_message(&url))
            .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use chrono::Weekday;
    use foundation_configuration::Secret;
    use foundation_recurring_job::Job;
    use mockito::{Matcher, Mock, Server, ServerGuard};
    use serde_json::json;
    use sqlx::PgPool;

    use crate::config::{OpenRouterConfig, ScheduleConfig, TelegramConfig};
    use crate::openrouter::OpenRouterClient;
    use crate::persistence::Role;
    use crate::telegram::TelegramClient;
    use crate::weekly_job::WeeklyRecipes;

    fn job(pool: PgPool, openrouter: &ServerGuard, telegram: &ServerGuard) -> WeeklyRecipes {
        let openrouter = OpenRouterClient::new(&OpenRouterConfig {
            api_key: Secret::from("key".to_owned()),
            model: "test/model".to_owned(),
            base_url: openrouter.url(),
        })
        .unwrap();

        let telegram = TelegramClient::new(&TelegramConfig {
            bot_token: Secret::from("token".to_owned()),
            chat_id: "1".to_owned(),
            base_url: telegram.url(),
        })
        .unwrap();

        let schedule = ScheduleConfig {
            weekday: Weekday::Sun,
            hour: 9,
            minute: 0,
        };

        WeeklyRecipes::new(pool, openrouter, telegram, "https://palate.test/", schedule)
    }

    async fn openrouter_ok(server: &mut ServerGuard) -> Mock {
        server
            .mock("POST", "/chat/completions")
            .with_header("content-type", "application/json")
            .with_body(
                json!({"choices": [{"message": {"role": "assistant", "content": "ideas"}}]})
                    .to_string(),
            )
            .create_async()
            .await
    }

    #[sqlx::test]
    async fn stores_conversation_and_sends_link(pool: PgPool) {
        let mut openrouter = Server::new_async().await;
        let mut telegram = Server::new_async().await;

        let completion = openrouter_ok(&mut openrouter).await;

        // The link is only known once the conversation has been created, so match on its shape
        let notification = telegram
            .mock("POST", "/bottoken/sendMessage")
            .match_body(Matcher::Regex(
                r#"<a href=\\"https://palate\.test/conversations/[0-9a-f-]+\\">"#.to_owned(),
            ))
            .with_header("content-type", "application/json")
            .with_body(json!({"ok": true}).to_string())
            .create_async()
            .await;

        job(pool.clone(), &openrouter, &telegram)
            .run()
            .await
            .unwrap();

        completion.assert_async().await;
        notification.assert_async().await;

        let conversations = crate::persistence::select_conversations(&pool)
            .await
            .unwrap();
        assert_eq!(conversations.len(), 1);

        let uid = conversations[0].conversation_uid;
        let conversation = crate::persistence::select_conversation(&pool, uid)
            .await
            .unwrap()
            .unwrap();

        let roles: Vec<_> = conversation.messages.iter().map(|m| m.role).collect();
        assert_eq!(roles, vec![Role::System, Role::User, Role::Assistant]);
        assert_eq!(conversation.messages[2].content, "ideas");
    }

    #[sqlx::test]
    async fn failed_model_calls_do_not_notify(pool: PgPool) {
        let mut openrouter = Server::new_async().await;
        let mut telegram = Server::new_async().await;

        openrouter
            .mock("POST", "/chat/completions")
            .with_status(500)
            .create_async()
            .await;

        let notification = telegram
            .mock("POST", "/bottoken/sendMessage")
            .expect(0)
            .create_async()
            .await;

        assert!(
            job(pool.clone(), &openrouter, &telegram)
                .run()
                .await
                .is_err()
        );
        notification.assert_async().await;

        // The prompts are kept so the failure can be investigated
        let conversations = crate::persistence::select_conversations(&pool)
            .await
            .unwrap();
        assert_eq!(conversations.len(), 1);
    }
}
