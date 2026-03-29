use chrono::{DateTime, Duration, NaiveDate, Utc};
use color_eyre::eyre::Result;
use sqlx::PgPool;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum EventType {
    Inserted,
    Removed,
}

impl TryFrom<&str> for EventType {
    type Error = color_eyre::eyre::Error;

    fn try_from(s: &str) -> Result<Self> {
        match s {
            "Inserted" => Ok(Self::Inserted),
            "Removed" => Ok(Self::Removed),
            other => Err(color_eyre::eyre::eyre!("unknown event type: {other}")),
        }
    }
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
    let midnight_state = prev_event_type
        .as_deref()
        .map(EventType::try_from)
        .transpose()?;
    let was_inserted_at_midnight = midnight_state == Some(EventType::Inserted);

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
        match EventType::try_from(event.event_type.as_str())? {
            EventType::Inserted => {
                last_insert_time = Some(event.occurred_at);
                current_state = EventType::Inserted;
            }
            EventType::Removed => {
                if let Some(insert_time) = last_insert_time.take() {
                    wear_minutes += event
                        .occurred_at
                        .signed_duration_since(insert_time)
                        .num_minutes();
                }
                current_state = EventType::Removed;
            }
        }
    }

    // If currently inserted, add elapsed time since last insert
    if let Some(insert_time) = last_insert_time {
        wear_minutes += now.signed_duration_since(insert_time).num_minutes();
    }

    let elapsed_minutes = now.signed_duration_since(today_start).num_minutes();
    let out_minutes = (elapsed_minutes - wear_minutes).max(0);

    let is_on_track = out_minutes < 2 * 60;

    Ok(DailyStats {
        wear_minutes,
        out_minutes,
        is_on_track,
        current_state,
    })
}

pub struct DayHistory {
    pub date: NaiveDate,
    pub wear_minutes: i64,
    pub out_minutes: i64,
    pub is_on_track: bool,
}

#[tracing::instrument(skip(pool))]
pub async fn get_history(pool: &PgPool, now: DateTime<Utc>) -> Result<Vec<DayHistory>> {
    let all_events = sqlx::query!(
        r#"
            SELECT ret.name as event_type, re.occurred_at
            FROM retainer_event re
            JOIN retainer_event_type ret ON ret.id = re.event_type_id
            ORDER BY re.occurred_at ASC
        "#
    )
    .fetch_all(pool)
    .await?;

    if all_events.is_empty() {
        return Ok(vec![]);
    }

    let first_date = all_events[0].occurred_at.date_naive();
    let today_date = now.date_naive();

    let mut results = Vec::new();
    let mut carry_inserted = false;
    let mut event_idx = 0;
    let mut day = first_date;

    while day <= today_date {
        let day_start = day.and_hms_opt(0, 0, 0).unwrap().and_utc();
        let day_end = day_start + Duration::hours(24);
        let effective_end = if day == today_date { now } else { day_end };

        let mut wear_minutes: i64 = 0;
        let mut last_insert_time: Option<DateTime<Utc>> = if carry_inserted {
            Some(day_start)
        } else {
            None
        };
        let mut end_inserted = carry_inserted;

        while event_idx < all_events.len() && all_events[event_idx].occurred_at < day_end {
            let event = &all_events[event_idx];
            event_idx += 1;

            match EventType::try_from(event.event_type.as_str())? {
                EventType::Inserted => {
                    last_insert_time = Some(event.occurred_at);
                    end_inserted = true;
                }
                EventType::Removed => {
                    if let Some(insert_time) = last_insert_time.take() {
                        wear_minutes += event
                            .occurred_at
                            .signed_duration_since(insert_time)
                            .num_minutes();
                    }
                    end_inserted = false;
                }
            }
        }

        if let Some(insert_time) = last_insert_time {
            wear_minutes += effective_end
                .signed_duration_since(insert_time)
                .num_minutes();
        }

        let elapsed_minutes = effective_end.signed_duration_since(day_start).num_minutes();
        let out_minutes = (elapsed_minutes - wear_minutes).max(0);
        let is_on_track = out_minutes < 2 * 60;

        results.push(DayHistory {
            date: day,
            wear_minutes,
            out_minutes,
            is_on_track,
        });

        carry_inserted = end_inserted;
        day = day.succ_opt().unwrap();
    }

    results.reverse();
    Ok(results)
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
    use chrono::{NaiveDate, TimeZone, Utc};
    use color_eyre::eyre::Result;
    use sqlx::PgPool;

    use super::{EventType, get_daily_stats, get_history, record_event};

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
    async fn on_track_when_out_time_under_budget(pool: PgPool) -> Result<()> {
        // 2 hours into the day, retainer worn the whole time — 0 minutes out, on track
        let insert_at = today_start();
        record_event(&pool, EventType::Inserted, insert_at).await?;

        let now = today_start() + chrono::Duration::hours(2);
        let stats = get_daily_stats(&pool, today_start(), now).await?;

        assert!(stats.is_on_track);

        Ok(())
    }

    #[sqlx::test]
    async fn not_on_track_when_out_time_exceeds_budget(pool: PgPool) -> Result<()> {
        // 3 hours out with no wear — exceeds 2 hour budget
        let now = today_start() + chrono::Duration::hours(3);
        let stats = get_daily_stats(&pool, today_start(), now).await?;

        assert!(!stats.is_on_track);

        Ok(())
    }

    #[sqlx::test]
    async fn not_on_track_when_out_time_exactly_at_budget(pool: PgPool) -> Result<()> {
        // Exactly 2 hours out — threshold is strict (< 2h), so 2h exactly is not on track
        let now = today_start() + chrono::Duration::hours(2);
        let stats = get_daily_stats(&pool, today_start(), now).await?;

        assert!(!stats.is_on_track);

        Ok(())
    }

    // --- get_history tests ---

    #[sqlx::test]
    async fn no_events_returns_empty_history(pool: PgPool) -> Result<()> {
        let now = Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).unwrap();
        let history = get_history(&pool, now).await?;

        assert!(history.is_empty());

        Ok(())
    }

    #[sqlx::test]
    async fn past_day_computes_wear_and_out_time(pool: PgPool) -> Result<()> {
        // Insert at 8am, remove at 8pm — 12h wear, 12h out on a complete past day
        let day1 = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        record_event(
            &pool,
            EventType::Inserted,
            day1 + chrono::Duration::hours(8),
        )
        .await?;
        record_event(
            &pool,
            EventType::Removed,
            day1 + chrono::Duration::hours(20),
        )
        .await?;

        // now is day 2, so day 1 is a completed past day (effective_end = midnight)
        let now = Utc.with_ymd_and_hms(2026, 1, 2, 12, 0, 0).unwrap();
        let history = get_history(&pool, now).await?;

        // history[0] = Jan 2 (today), history[1] = Jan 1 (past)
        assert_eq!(history[1].wear_minutes, 12 * 60);
        assert_eq!(history[1].out_minutes, 12 * 60);

        Ok(())
    }

    #[sqlx::test]
    async fn entries_are_ordered_most_recent_first(pool: PgPool) -> Result<()> {
        let day1 = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let day2 = Utc.with_ymd_and_hms(2026, 1, 2, 0, 0, 0).unwrap();

        record_event(
            &pool,
            EventType::Inserted,
            day1 + chrono::Duration::hours(8),
        )
        .await?;
        record_event(
            &pool,
            EventType::Removed,
            day1 + chrono::Duration::hours(20),
        )
        .await?;
        record_event(
            &pool,
            EventType::Inserted,
            day2 + chrono::Duration::hours(8),
        )
        .await?;
        record_event(
            &pool,
            EventType::Removed,
            day2 + chrono::Duration::hours(20),
        )
        .await?;

        let now = Utc.with_ymd_and_hms(2026, 1, 2, 22, 0, 0).unwrap();
        let history = get_history(&pool, now).await?;

        assert_eq!(history.len(), 2);
        assert_eq!(
            history[0].date,
            NaiveDate::from_ymd_opt(2026, 1, 2).unwrap()
        );
        assert_eq!(
            history[1].date,
            NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()
        );

        Ok(())
    }

    #[sqlx::test]
    async fn inserted_state_carries_forward_across_midnight(pool: PgPool) -> Result<()> {
        let day1 = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let day2 = Utc.with_ymd_and_hms(2026, 1, 2, 0, 0, 0).unwrap();

        // Inserted at 9pm on day 1, removed at 8am on day 2
        record_event(
            &pool,
            EventType::Inserted,
            day1 + chrono::Duration::hours(21),
        )
        .await?;
        record_event(&pool, EventType::Removed, day2 + chrono::Duration::hours(8)).await?;

        let now = Utc.with_ymd_and_hms(2026, 1, 2, 12, 0, 0).unwrap();
        let history = get_history(&pool, now).await?;

        assert_eq!(history.len(), 2);
        // Day 1: wore from 9pm to midnight = 3h
        assert_eq!(history[1].wear_minutes, 3 * 60);
        // Day 2: state carried in, wore from midnight to 8am = 8h
        assert_eq!(history[0].wear_minutes, 8 * 60);

        Ok(())
    }

    #[sqlx::test]
    async fn gap_day_between_events_has_zero_wear_time(pool: PgPool) -> Result<()> {
        let day1 = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let day3 = Utc.with_ymd_and_hms(2026, 1, 3, 0, 0, 0).unwrap();

        record_event(
            &pool,
            EventType::Inserted,
            day1 + chrono::Duration::hours(8),
        )
        .await?;
        record_event(
            &pool,
            EventType::Removed,
            day1 + chrono::Duration::hours(20),
        )
        .await?;
        record_event(
            &pool,
            EventType::Inserted,
            day3 + chrono::Duration::hours(8),
        )
        .await?;
        record_event(
            &pool,
            EventType::Removed,
            day3 + chrono::Duration::hours(20),
        )
        .await?;

        let now = Utc.with_ymd_and_hms(2026, 1, 3, 22, 0, 0).unwrap();
        let history = get_history(&pool, now).await?;

        assert_eq!(history.len(), 3);
        // history[0] = Jan 3, [1] = Jan 2, [2] = Jan 1
        assert_eq!(
            history[1].date,
            NaiveDate::from_ymd_opt(2026, 1, 2).unwrap()
        );
        assert_eq!(history[1].wear_minutes, 0);
        assert_eq!(history[1].out_minutes, 24 * 60);

        Ok(())
    }

    #[sqlx::test]
    async fn past_day_is_on_track_when_out_time_under_budget(pool: PgPool) -> Result<()> {
        // 23 hours worn = 1 hour out, under the 2 hour budget
        let day1 = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        record_event(&pool, EventType::Inserted, day1).await?;
        record_event(
            &pool,
            EventType::Removed,
            day1 + chrono::Duration::hours(23),
        )
        .await?;

        let now = Utc.with_ymd_and_hms(2026, 1, 2, 12, 0, 0).unwrap();
        let history = get_history(&pool, now).await?;

        assert!(history[1].is_on_track);

        Ok(())
    }

    #[sqlx::test]
    async fn past_day_is_not_on_track_when_out_time_exceeds_budget(pool: PgPool) -> Result<()> {
        // 20 hours worn = 4 hours out, exceeds the 2 hour budget
        let day1 = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        record_event(&pool, EventType::Inserted, day1).await?;
        record_event(
            &pool,
            EventType::Removed,
            day1 + chrono::Duration::hours(20),
        )
        .await?;

        let now = Utc.with_ymd_and_hms(2026, 1, 2, 12, 0, 0).unwrap();
        let history = get_history(&pool, now).await?;

        assert!(!history[1].is_on_track);

        Ok(())
    }
}
