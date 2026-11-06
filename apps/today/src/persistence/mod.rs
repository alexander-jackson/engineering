use chrono::{NaiveDate, NaiveDateTime, Utc};
use color_eyre::eyre::Result;
use pulldown_cmark::{Event, Parser};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::uid::ItemUid;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ItemState {
    Checked,
    Unchecked,
    Deleted,
}

impl ItemState {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Checked => "Checked",
            Self::Unchecked => "Unchecked",
            Self::Deleted => "Deleted",
        }
    }
}

impl From<String> for ItemState {
    fn from(value: String) -> Self {
        match value.as_str() {
            "Checked" => Self::Checked,
            "Unchecked" => Self::Unchecked,
            "Deleted" => Self::Deleted,
            _ => panic!("invalid argument {value}"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Content(String);

impl From<String> for Content {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl Serialize for Content {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut rendered = String::new();

        for event in Parser::new(&self.0) {
            match event {
                Event::Text(value) => rendered.push_str(&value),
                Event::Code(value) => {
                    rendered.push_str("<code>");
                    rendered.push_str(&value);
                    rendered.push_str("</code>");
                }
                _ => {}
            };
        }

        serializer.serialize_str(&rendered)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Item {
    pub item_uid: ItemUid,
    pub content: Content,
    // The unrendered markdown, used to prefill the edit field
    pub source: String,
    pub state: ItemState,
}

#[tracing::instrument(skip(pool))]
pub async fn select_items(pool: &PgPool, date: NaiveDate) -> Result<Vec<Item>> {
    let items = sqlx::query_as!(
        Item,
        r#"
            WITH items_with_states AS (
                SELECT DISTINCT ON (i.id)
                    i.created_at,
                    i.item_uid,
                    latest_content.content,
                    iet.name AS state
                FROM item i
                JOIN item_event ie ON i.id = ie.item_id
                JOIN item_event_type iet ON iet.id = ie.event_type_id
                JOIN LATERAL (
                    SELECT ic.content
                    FROM item_content ic
                    WHERE ic.item_id = i.id
                    ORDER BY ic.recorded_at DESC, ic.id DESC
                    LIMIT 1
                ) latest_content ON TRUE
                WHERE (i.created_at AT TIME ZONE 'UTC' AT TIME ZONE 'Europe/London')::date = $1
                ORDER BY i.id, i.created_at, ie.occurred_at DESC
            )
            SELECT item_uid, content, content AS source, state
            FROM items_with_states
            WHERE state != 'Deleted'
            ORDER BY created_at
        "#,
        date,
    )
    .fetch_all(pool)
    .await?;

    Ok(items)
}

#[tracing::instrument(skip(pool, content))]
pub async fn create_item(
    pool: &PgPool,
    item_uid: ItemUid,
    content: &str,
    created_at: NaiveDateTime,
) -> Result<()> {
    let mut tx = pool.begin().await?;

    let item_id = sqlx::query_scalar!(
        r#"
            INSERT INTO item (item_uid, created_at)
            VALUES ($1, $2)
            RETURNING id
        "#,
        *item_uid,
        created_at,
    )
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query!(
        r#"
            INSERT INTO item_content (item_id, content, recorded_at)
            VALUES ($1, $2, $3)
        "#,
        item_id,
        content,
        created_at,
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query!(
        r#"
            INSERT INTO item_event (item_id, event_type_id, occurred_at)
            VALUES (
                $1,
                (SELECT id FROM item_event_type WHERE name = 'Unchecked'),
                $2
            )
        "#,
        item_id,
        created_at,
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(())
}

#[tracing::instrument(skip(pool))]
pub async fn update_item(pool: &PgPool, item_uid: ItemUid, state: ItemState) -> Result<()> {
    let mut tx = pool.begin().await?;
    let now = Utc::now().naive_local();

    sqlx::query!(
        r#"
            INSERT INTO item_event (item_id, event_type_id, occurred_at)
            VALUES (
                (
                    SELECT i.id
                    FROM item i
                    WHERE i.item_uid = $1
                ),
                (SELECT id FROM item_event_type WHERE name = $2),
                $3
            )
        "#,
        *item_uid,
        state.as_str(),
        now,
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(())
}

// Content is append-only, so an edit adds a new version rather than changing the old one
#[tracing::instrument(skip(pool, content))]
pub async fn edit_item(pool: &PgPool, item_uid: ItemUid, content: &str) -> Result<()> {
    let now = Utc::now().naive_local();

    let result = sqlx::query!(
        r#"
            INSERT INTO item_content (item_id, content, recorded_at)
            SELECT i.id, $2, $3
            FROM item i
            WHERE i.item_uid = $1
        "#,
        *item_uid,
        content,
        now,
    )
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(color_eyre::eyre::eyre!(
            "no item exists with uid {item_uid}"
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests;
