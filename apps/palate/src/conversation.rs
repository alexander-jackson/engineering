use chrono::Utc;
use color_eyre::eyre::{Result, eyre};
use sqlx::PgPool;

use crate::openrouter::OpenRouterClient;
use crate::persistence::Role;
use crate::uid::ConversationUid;

/// Replays the stored history to the model and stores the reply as the next message.
#[tracing::instrument(skip(pool, openrouter))]
pub async fn generate_reply(
    pool: &PgPool,
    openrouter: &OpenRouterClient,
    conversation_uid: ConversationUid,
) -> Result<()> {
    let conversation = crate::persistence::select_conversation(pool, conversation_uid)
        .await?
        .ok_or_else(|| eyre!("conversation {conversation_uid} does not exist"))?;

    let reply = openrouter.complete(&conversation.messages).await?;

    crate::persistence::insert_message(pool, conversation_uid, Role::Assistant, &reply, Utc::now())
        .await?;

    Ok(())
}
