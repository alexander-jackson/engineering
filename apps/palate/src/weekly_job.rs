use chrono::Utc;
use color_eyre::eyre::Result;
use foundation_recurring_job::{Job, Schedule};
use sqlx::PgPool;

use crate::config::ScheduleConfig;
use crate::openrouter::OpenRouterClient;
use crate::persistence::Role;
use crate::prompt::{SYSTEM_PROMPT, WEEKLY_PROMPT};
use crate::telegram::{Notifier, conversation_ready_message};
use crate::uid::ConversationUid;

pub struct WeeklyRecipes<N> {
    pool: PgPool,
    openrouter: OpenRouterClient,
    notifier: N,
    base_url: String,
    schedule: ScheduleConfig,
}

impl<N: Notifier> WeeklyRecipes<N> {
    pub fn new(
        pool: PgPool,
        openrouter: OpenRouterClient,
        notifier: N,
        base_url: &str,
        schedule: ScheduleConfig,
    ) -> Self {
        Self {
            pool,
            openrouter,
            notifier,
            base_url: base_url.trim_end_matches('/').to_owned(),
            schedule,
        }
    }
}

impl<N: Notifier> Job for WeeklyRecipes<N> {
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

        self.notifier
            .send_message(&conversation_ready_message(&url))
            .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use chrono::Weekday;
    use color_eyre::eyre::Result;
    use foundation_configuration::Secret;
    use foundation_recurring_job::Job;
    use mockito::{Mock, Server, ServerGuard};
    use serde_json::json;
    use sqlx::PgPool;

    use crate::config::{OpenRouterConfig, ScheduleConfig};
    use crate::openrouter::OpenRouterClient;
    use crate::persistence::Role;
    use crate::telegram::Notifier;
    use crate::weekly_job::WeeklyRecipes;

    /// Records the messages it is asked to send instead of delivering them.
    #[derive(Clone, Default)]
    struct RecordingNotifier {
        messages: Arc<Mutex<Vec<String>>>,
    }

    impl RecordingNotifier {
        fn messages(&self) -> Vec<String> {
            self.messages.lock().unwrap().clone()
        }
    }

    impl Notifier for RecordingNotifier {
        async fn send_message(&self, text: &str) -> Result<()> {
            self.messages.lock().unwrap().push(text.to_owned());

            Ok(())
        }
    }

    fn job(
        pool: PgPool,
        openrouter: &ServerGuard,
        notifier: RecordingNotifier,
    ) -> WeeklyRecipes<RecordingNotifier> {
        let openrouter = OpenRouterClient::new(&OpenRouterConfig {
            api_key: Secret::from("key".to_owned()),
            model: "test/model".to_owned(),
            base_url: openrouter.url(),
        })
        .unwrap();

        let schedule = ScheduleConfig {
            weekday: Weekday::Sun,
            hour: 9,
            minute: 0,
        };

        WeeklyRecipes::new(pool, openrouter, notifier, "https://palate.test/", schedule)
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
        let notifier = RecordingNotifier::default();

        let completion = openrouter_ok(&mut openrouter).await;

        job(pool.clone(), &openrouter, notifier.clone())
            .run()
            .await
            .unwrap();

        completion.assert_async().await;

        let conversations = crate::persistence::select_conversations(&pool)
            .await
            .unwrap();
        assert_eq!(conversations.len(), 1);

        let uid = conversations[0].conversation_uid;

        // The link points at the conversation that was just stored
        let messages = notifier.messages();
        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains(&format!(
            "<a href=\"https://palate.test/conversations/{uid}\">"
        )));

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
        let notifier = RecordingNotifier::default();

        openrouter
            .mock("POST", "/chat/completions")
            .with_status(500)
            .create_async()
            .await;

        assert!(
            job(pool.clone(), &openrouter, notifier.clone())
                .run()
                .await
                .is_err()
        );
        assert!(notifier.messages().is_empty());

        // The prompts are kept so the failure can be investigated
        let conversations = crate::persistence::select_conversations(&pool)
            .await
            .unwrap();
        assert_eq!(conversations.len(), 1);
    }
}
