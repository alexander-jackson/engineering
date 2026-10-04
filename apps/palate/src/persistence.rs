use chrono::{DateTime, Utc};
use color_eyre::eyre::{Result, eyre};
use sqlx::PgPool;

use crate::uid::ConversationUid;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Role {
    System,
    User,
    Assistant,
}

impl Role {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::System => "system",
            Self::User => "user",
            Self::Assistant => "assistant",
        }
    }
}

impl TryFrom<&str> for Role {
    type Error = color_eyre::Report;

    fn try_from(value: &str) -> Result<Self> {
        match value {
            "system" => Ok(Self::System),
            "user" => Ok(Self::User),
            "assistant" => Ok(Self::Assistant),
            other => Err(eyre!("unknown conversation message role '{other}'")),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Message {
    pub role: Role,
    pub content: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct Conversation {
    pub conversation_uid: ConversationUid,
    pub created_at: DateTime<Utc>,
    pub messages: Vec<Message>,
}

#[derive(Clone, Debug)]
pub struct ConversationSummary {
    pub conversation_uid: ConversationUid,
    pub created_at: DateTime<Utc>,
}

/// Creates a conversation and its initial messages atomically.
#[tracing::instrument(skip(pool, messages))]
pub async fn create_conversation(
    pool: &PgPool,
    conversation_uid: ConversationUid,
    messages: &[(Role, &str)],
    now: DateTime<Utc>,
) -> Result<()> {
    let mut tx = pool.begin().await?;

    sqlx::query!(
        r#"
        INSERT INTO conversation (conversation_uid, created_at)
        VALUES ($1, $2)
        "#,
        *conversation_uid,
        now
    )
    .execute(&mut *tx)
    .await?;

    for (role, content) in messages {
        insert_message_with(&mut tx, conversation_uid, *role, content, now).await?;
    }

    tx.commit().await?;

    Ok(())
}

#[tracing::instrument(skip(pool, content))]
pub async fn insert_message(
    pool: &PgPool,
    conversation_uid: ConversationUid,
    role: Role,
    content: &str,
    now: DateTime<Utc>,
) -> Result<()> {
    let mut conn = pool.acquire().await?;

    insert_message_with(&mut conn, conversation_uid, role, content, now).await
}

async fn insert_message_with(
    conn: &mut sqlx::PgConnection,
    conversation_uid: ConversationUid,
    role: Role,
    content: &str,
    now: DateTime<Utc>,
) -> Result<()> {
    let result = sqlx::query!(
        r#"
        INSERT INTO conversation_message (conversation_id, conversation_message_role_id, content, created_at)
        SELECT c.id, r.id, $3, $4
        FROM conversation c
        JOIN conversation_message_role r ON r.name = $2
        WHERE c.conversation_uid = $1
        "#,
        *conversation_uid,
        role.as_str(),
        content,
        now
    )
    .execute(conn)
    .await?;

    if result.rows_affected() == 0 {
        return Err(eyre!("conversation {conversation_uid} does not exist"));
    }

    Ok(())
}

#[tracing::instrument(skip(pool))]
pub async fn select_conversation(
    pool: &PgPool,
    conversation_uid: ConversationUid,
) -> Result<Option<Conversation>> {
    let Some(conversation) = sqlx::query!(
        r#"
        SELECT created_at
        FROM conversation
        WHERE conversation_uid = $1
        "#,
        *conversation_uid
    )
    .fetch_optional(pool)
    .await?
    else {
        return Ok(None);
    };

    let rows = sqlx::query!(
        r#"
        SELECT r.name AS role, m.content, m.created_at
        FROM conversation_message m
        JOIN conversation c ON c.id = m.conversation_id
        JOIN conversation_message_role r ON r.id = m.conversation_message_role_id
        WHERE c.conversation_uid = $1
        ORDER BY m.id
        "#,
        *conversation_uid
    )
    .fetch_all(pool)
    .await?;

    let messages = rows
        .into_iter()
        .map(|row| {
            Ok(Message {
                role: Role::try_from(row.role.as_str())?,
                content: row.content,
                created_at: row.created_at,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(Some(Conversation {
        conversation_uid,
        created_at: conversation.created_at,
        messages,
    }))
}

#[tracing::instrument(skip(pool))]
pub async fn select_conversations(pool: &PgPool) -> Result<Vec<ConversationSummary>> {
    let rows = sqlx::query!(
        r#"
        SELECT conversation_uid, created_at
        FROM conversation
        ORDER BY created_at DESC
        "#
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| ConversationSummary {
            conversation_uid: row.conversation_uid.into(),
            created_at: row.created_at,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use sqlx::PgPool;

    use crate::persistence::select_conversations;
    use crate::persistence::{Role, create_conversation, insert_message, select_conversation};
    use crate::uid::ConversationUid;

    #[sqlx::test]
    async fn conversations_keep_messages_in_order(pool: PgPool) -> color_eyre::Result<()> {
        let uid = ConversationUid::new();
        let now = Utc::now();

        create_conversation(
            &pool,
            uid,
            &[(Role::System, "be helpful"), (Role::User, "hello")],
            now,
        )
        .await?;
        insert_message(&pool, uid, Role::Assistant, "hi there", now).await?;

        let conversation = select_conversation(&pool, uid).await?.expect("missing");
        let messages: Vec<_> = conversation
            .messages
            .iter()
            .map(|m| (m.role, m.content.as_str()))
            .collect();

        assert_eq!(
            messages,
            vec![
                (Role::System, "be helpful"),
                (Role::User, "hello"),
                (Role::Assistant, "hi there"),
            ]
        );

        Ok(())
    }

    #[sqlx::test]
    async fn missing_conversations_are_none(pool: PgPool) -> color_eyre::Result<()> {
        let conversation = select_conversation(&pool, ConversationUid::new()).await?;

        assert!(conversation.is_none());

        Ok(())
    }

    #[sqlx::test]
    async fn cannot_add_messages_to_missing_conversations(pool: PgPool) {
        let result = insert_message(
            &pool,
            ConversationUid::new(),
            Role::User,
            "hello",
            Utc::now(),
        )
        .await;

        assert!(result.is_err());
    }

    #[sqlx::test]
    async fn conversations_are_listed_newest_first(pool: PgPool) -> color_eyre::Result<()> {
        let older = ConversationUid::new();
        let newer = ConversationUid::new();
        let now = Utc::now();

        create_conversation(&pool, older, &[], now - chrono::Duration::days(7)).await?;
        create_conversation(&pool, newer, &[], now).await?;

        let uids: Vec<_> = select_conversations(&pool)
            .await?
            .into_iter()
            .map(|c| c.conversation_uid)
            .collect();

        assert_eq!(uids, vec![newer, older]);

        Ok(())
    }
}
