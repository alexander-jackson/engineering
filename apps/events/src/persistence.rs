use chrono::{DateTime, Duration, Utc};
use color_eyre::eyre::Result;
use sqlx::PgPool;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum EventType {
    Inserted,
    Removed,
}

pub struct DailyStats {
    pub wear_minutes: i64,
    pub out_minutes: i64,
    pub is_on_track: bool,
    pub current_state: EventType,
}

#[tracing::instrument(skip(pool))]
pub async fn get_daily_stats(
    pool: &PgPool,
    today_start: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Result<DailyStats> {
    // Get state just before today (to know if retainer was in/out at midnight)
    let prev_event_type = sqlx::query_scalar!(
        r#"
            SELECT ret.name
            FROM retainer_event re
            JOIN retainer_event_type ret ON ret.id = re.event_type_id
            WHERE re.occurred_at < $1
            ORDER BY re.occurred_at DESC
            LIMIT 1
        "#,
        today_start
    )
    .fetch_optional(pool)
    .await?;

    // Get all events today in chronological order
    let today_events = sqlx::query!(
        r#"
            SELECT ret.name as event_type, re.occurred_at
            FROM retainer_event re
            JOIN retainer_event_type ret ON ret.id = re.event_type_id
            WHERE re.occurred_at >= $1
            ORDER BY re.occurred_at ASC
        "#,
        today_start
    )
    .fetch_all(pool)
    .await?;

    // Determine state at midnight
    let was_inserted_at_midnight = prev_event_type.as_deref() == Some("Inserted");

    let mut last_insert_time: Option<DateTime<Utc>> = if was_inserted_at_midnight {
        Some(today_start)
    } else {
        None
    };

    let mut wear_minutes: i64 = 0;
    let mut current_state = if was_inserted_at_midnight {
        EventType::Inserted
    } else {
        EventType::Removed
    };

    for event in &today_events {
        match event.event_type.as_str() {
            "Inserted" => {
                last_insert_time = Some(event.occurred_at);
                current_state = EventType::Inserted;
            }
            "Removed" => {
                if let Some(insert_time) = last_insert_time.take() {
                    wear_minutes += event
                        .occurred_at
                        .signed_duration_since(insert_time)
                        .num_minutes();
                }
                current_state = EventType::Removed;
            }
            _ => {}
        }
    }

    // If currently inserted, add elapsed time since last insert
    if let Some(insert_time) = last_insert_time {
        wear_minutes += now.signed_duration_since(insert_time).num_minutes();
    }

    let elapsed_minutes = now.signed_duration_since(today_start).num_minutes();
    let out_minutes = (elapsed_minutes - wear_minutes).max(0);

    // Minutes remaining in today
    let end_of_day = today_start + Duration::hours(24);
    let remaining_minutes = end_of_day.signed_duration_since(now).num_minutes().max(0);

    let is_on_track = wear_minutes + remaining_minutes >= 22 * 60;

    Ok(DailyStats {
        wear_minutes,
        out_minutes,
        is_on_track,
        current_state,
    })
}

#[tracing::instrument(skip(pool))]
pub async fn record_event(
    pool: &PgPool,
    event_type: EventType,
    occurred_at: DateTime<Utc>,
) -> Result<()> {
    let event_type_name = match event_type {
        EventType::Inserted => "Inserted",
        EventType::Removed => "Removed",
    };

    sqlx::query!(
        r#"
            INSERT INTO retainer_event (event_type_id, occurred_at)
            VALUES ((SELECT id FROM retainer_event_type WHERE name = $1), $2)
        "#,
        event_type_name,
        occurred_at
    )
    .execute(pool)
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use color_eyre::eyre::Result;
    use sqlx::PgPool;

    use super::{EventType, get_daily_stats, record_event};

    fn today_start() -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap()
    }

    #[sqlx::test]
    async fn no_events_defaults_to_removed_with_zero_wear_time(pool: PgPool) -> Result<()> {
        let now = today_start() + chrono::Duration::hours(10);
        let stats = get_daily_stats(&pool, today_start(), now).await?;

        assert_eq!(stats.current_state, EventType::Removed);
        assert_eq!(stats.wear_minutes, 0);

        Ok(())
    }

    #[sqlx::test]
    async fn recording_insert_sets_inserted_state(pool: PgPool) -> Result<()> {
        let t = today_start() + chrono::Duration::hours(8);
        record_event(&pool, EventType::Inserted, t).await?;

        let now = t + chrono::Duration::minutes(1);
        let stats = get_daily_stats(&pool, today_start(), now).await?;

        assert_eq!(stats.current_state, EventType::Inserted);

        Ok(())
    }

    #[sqlx::test]
    async fn recording_remove_after_insert_sets_removed_state(pool: PgPool) -> Result<()> {
        let t = today_start() + chrono::Duration::hours(8);
        record_event(&pool, EventType::Inserted, t).await?;
        record_event(&pool, EventType::Removed, t + chrono::Duration::hours(1)).await?;

        let now = t + chrono::Duration::hours(2);
        let stats = get_daily_stats(&pool, today_start(), now).await?;

        assert_eq!(stats.current_state, EventType::Removed);

        Ok(())
    }

    #[sqlx::test]
    async fn wear_time_counts_completed_insert_remove_interval(pool: PgPool) -> Result<()> {
        let insert_at = today_start() + chrono::Duration::hours(8);
        let remove_at = insert_at + chrono::Duration::minutes(90);

        record_event(&pool, EventType::Inserted, insert_at).await?;
        record_event(&pool, EventType::Removed, remove_at).await?;

        let now = remove_at + chrono::Duration::hours(1);
        let stats = get_daily_stats(&pool, today_start(), now).await?;

        assert_eq!(stats.wear_minutes, 90);

        Ok(())
    }

    #[sqlx::test]
    async fn wear_time_includes_open_interval_since_last_insert(pool: PgPool) -> Result<()> {
        let insert_at = today_start() + chrono::Duration::hours(8);
        record_event(&pool, EventType::Inserted, insert_at).await?;

        let now = insert_at + chrono::Duration::minutes(45);
        let stats = get_daily_stats(&pool, today_start(), now).await?;

        assert_eq!(stats.wear_minutes, 45);

        Ok(())
    }

    #[sqlx::test]
    async fn wear_time_sums_multiple_intervals(pool: PgPool) -> Result<()> {
        let t = today_start();
        // First session: 1 hour
        record_event(&pool, EventType::Inserted, t + chrono::Duration::hours(8)).await?;
        record_event(&pool, EventType::Removed, t + chrono::Duration::hours(9)).await?;
        // Second session: 30 minutes
        record_event(&pool, EventType::Inserted, t + chrono::Duration::hours(10)).await?;
        record_event(
            &pool,
            EventType::Removed,
            t + chrono::Duration::minutes(630),
        )
        .await?;

        let now = t + chrono::Duration::hours(12);
        let stats = get_daily_stats(&pool, today_start(), now).await?;

        assert_eq!(stats.wear_minutes, 90);

        Ok(())
    }

    #[sqlx::test]
    async fn insert_event_before_today_contributes_wear_time_from_midnight(
        pool: PgPool,
    ) -> Result<()> {
        // Retainer was inserted yesterday evening
        let yesterday_evening = today_start() - chrono::Duration::hours(2);
        record_event(&pool, EventType::Inserted, yesterday_evening).await?;

        // Two hours into today, still inserted
        let now = today_start() + chrono::Duration::hours(2);
        let stats = get_daily_stats(&pool, today_start(), now).await?;

        assert_eq!(stats.current_state, EventType::Inserted);
        assert_eq!(stats.wear_minutes, 120);

        Ok(())
    }

    #[sqlx::test]
    async fn remove_event_before_today_gives_removed_state_at_midnight(pool: PgPool) -> Result<()> {
        // Inserted and removed yesterday
        let t = today_start() - chrono::Duration::hours(5);
        record_event(&pool, EventType::Inserted, t).await?;
        record_event(&pool, EventType::Removed, t + chrono::Duration::hours(1)).await?;

        let now = today_start() + chrono::Duration::hours(2);
        let stats = get_daily_stats(&pool, today_start(), now).await?;

        assert_eq!(stats.current_state, EventType::Removed);
        assert_eq!(stats.wear_minutes, 0);

        Ok(())
    }

    #[sqlx::test]
    async fn on_track_when_enough_remaining_time_to_hit_target(pool: PgPool) -> Result<()> {
        // 2 hours into the day, 0 minutes worn — 22 * 60 = 1320 minutes needed,
        // 22 hours remaining, so still on track
        let now = today_start() + chrono::Duration::hours(2);
        let stats = get_daily_stats(&pool, today_start(), now).await?;

        assert!(stats.is_on_track);

        Ok(())
    }

    #[sqlx::test]
    async fn not_on_track_when_too_little_time_remains(pool: PgPool) -> Result<()> {
        // 23 hours into the day, 0 minutes worn — only 60 minutes left, can't hit target
        let now = today_start() + chrono::Duration::hours(23);
        let stats = get_daily_stats(&pool, today_start(), now).await?;

        assert!(!stats.is_on_track);

        Ok(())
    }

    #[sqlx::test]
    async fn on_track_when_wear_time_plus_remaining_meets_target(pool: PgPool) -> Result<()> {
        // 23 hours into the day, retainer worn for 21 hours and 30 mins — 30 mins left,
        // total = 21h30 + 30min = 22h, exactly on target
        let insert_at = today_start();
        let remove_at = insert_at + chrono::Duration::minutes(21 * 60 + 30);
        record_event(&pool, EventType::Inserted, insert_at).await?;
        record_event(&pool, EventType::Removed, remove_at).await?;

        let now = today_start() + chrono::Duration::hours(23);
        let stats = get_daily_stats(&pool, today_start(), now).await?;

        assert!(stats.is_on_track);

        Ok(())
    }
}
