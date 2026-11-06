use std::collections::HashSet;

use chrono::{Duration, NaiveDate, NaiveDateTime, Utc};
use color_eyre::eyre::Result;
use sqlx::PgPool;

use crate::persistence::{Content, ItemState};
use crate::uid::ItemUid;

async fn create_item(pool: &PgPool, content: &str, created_at: NaiveDateTime) -> Result<ItemUid> {
    let item_uid = ItemUid::new();

    super::create_item(pool, item_uid, content, created_at).await?;

    Ok(item_uid)
}

#[sqlx::test]
fn items_are_only_returned_for_the_current_day(pool: PgPool) -> Result<()> {
    let now = Utc::now().naive_local();
    let yesterday = now - Duration::days(1);
    let today = crate::clock::today();

    // Insert an item for the current day
    let item_uid1 = create_item(&pool, "Today", now).await?;

    // Insert an item for yesterday
    let item_uid2 = create_item(&pool, "Yesterday", yesterday).await?;

    // Fetch all the items for today
    let items: HashSet<ItemUid> = super::select_items(&pool, today)
        .await?
        .iter()
        .map(|item| item.item_uid)
        .collect();

    assert!(items.contains(&item_uid1));
    assert!(!items.contains(&item_uid2));

    Ok(())
}

#[sqlx::test]
fn days_change_at_london_midnight(pool: PgPool) -> Result<()> {
    // 23:30 UTC on 1 July is already 00:30 on 2 July in London (British summer time)
    let created_at = NaiveDate::from_ymd_opt(2026, 7, 1)
        .unwrap()
        .and_hms_opt(23, 30, 0)
        .unwrap();
    let item_uid = create_item(&pool, "Late", created_at).await?;

    let first = NaiveDate::from_ymd_opt(2026, 7, 1).unwrap();
    let second = NaiveDate::from_ymd_opt(2026, 7, 2).unwrap();

    let on_first: HashSet<ItemUid> = super::select_items(&pool, first)
        .await?
        .iter()
        .map(|item| item.item_uid)
        .collect();

    let on_second: HashSet<ItemUid> = super::select_items(&pool, second)
        .await?
        .iter()
        .map(|item| item.item_uid)
        .collect();

    assert!(!on_first.contains(&item_uid));
    assert!(on_second.contains(&item_uid));

    Ok(())
}

#[sqlx::test]
fn item_states_can_be_modified(pool: PgPool) -> Result<()> {
    let now = Utc::now().naive_local();
    let today = crate::clock::today();

    // Insert an item to modify
    let item_uid = create_item(&pool, "Content", now).await?;

    // Check it's currently unchecked
    let items = super::select_items(&pool, today).await?;
    assert_eq!(items[0].state, ItemState::Unchecked);

    // Modify the item
    super::update_item(&pool, item_uid, ItemState::Checked).await?;

    // Check the new state is reflected
    let items = super::select_items(&pool, today).await?;
    assert_eq!(items[0].state, ItemState::Checked);

    // Update it back to be unchecked
    super::update_item(&pool, item_uid, ItemState::Unchecked).await?;

    // Check it's back to what it was before
    let items = super::select_items(&pool, today).await?;
    assert_eq!(items[0].state, ItemState::Unchecked);

    Ok(())
}

#[sqlx::test]
fn deleted_items_do_not_get_returned(pool: PgPool) -> Result<()> {
    let now = Utc::now().naive_local();
    let today = crate::clock::today();

    // Create 2 items
    let item_uid1 = create_item(&pool, "First", now).await?;
    let item_uid2 = create_item(&pool, "Second", now).await?;

    // Mark the first as deleted
    super::update_item(&pool, item_uid1, ItemState::Deleted).await?;

    // Fetch all the items and assert we just have the second
    let items: HashSet<_> = super::select_items(&pool, today)
        .await?
        .iter()
        .map(|i| i.item_uid)
        .collect();

    assert!(!items.contains(&item_uid1));
    assert!(items.contains(&item_uid2));

    Ok(())
}

#[sqlx::test]
fn edited_content_replaces_the_original(pool: PgPool) -> Result<()> {
    let now = Utc::now().naive_local();
    let today = crate::clock::today();

    let item_uid = create_item(&pool, "Buy mlik", now).await?;
    super::edit_item(&pool, item_uid, "Buy milk").await?;

    let items = super::select_items(&pool, today).await?;

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].content, Content::from("Buy milk".to_string()));

    Ok(())
}

#[sqlx::test]
fn edits_keep_the_earlier_versions_of_the_content(pool: PgPool) -> Result<()> {
    let now = Utc::now().naive_local();
    let item_uid = create_item(&pool, "Original", now).await?;

    super::edit_item(&pool, item_uid, "First edit").await?;
    super::edit_item(&pool, item_uid, "Second edit").await?;

    let versions: Vec<String> = sqlx::query_scalar(
        r#"
            SELECT ic.content
            FROM item_content ic
            JOIN item i ON i.id = ic.item_id
            WHERE i.item_uid = $1
            ORDER BY ic.recorded_at, ic.id
        "#,
    )
    .bind(*item_uid)
    .fetch_all(&pool)
    .await?;

    assert_eq!(versions, vec!["Original", "First edit", "Second edit"]);

    Ok(())
}

#[sqlx::test]
fn edits_do_not_move_items_in_the_list(pool: PgPool) -> Result<()> {
    let today = crate::clock::today();
    let first_at = Utc::now().naive_local();
    let second_at = first_at + Duration::seconds(1);

    let first = create_item(&pool, "First", first_at).await?;
    let second = create_item(&pool, "Second", second_at).await?;

    super::edit_item(&pool, first, "First, edited").await?;

    let uids: Vec<ItemUid> = super::select_items(&pool, today)
        .await?
        .iter()
        .map(|item| item.item_uid)
        .collect();

    assert_eq!(uids, vec![first, second]);

    Ok(())
}

#[sqlx::test]
fn editing_an_unknown_item_fails(pool: PgPool) -> Result<()> {
    let result = super::edit_item(&pool, ItemUid::new(), "Nothing").await;

    assert!(result.is_err());

    Ok(())
}
