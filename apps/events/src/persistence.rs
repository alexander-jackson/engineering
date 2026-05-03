use chrono::{DateTime, Duration, NaiveDate, Utc};
use color_eyre::eyre::{Result, eyre};
use itertools::Itertools;
use sqlx::PgPool;

#[derive(Copy, Clone, Debug, Eq, PartialEq, sqlx::Type)]
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
            other => Err(eyre!("unknown event type: {other}")),
        }
    }
}

pub struct DailyStats {
    pub wear_minutes: i64,
    pub out_minutes: i64,
    pub is_on_track: bool,
    pub current_state: EventType,
    pub latest_event_time: DateTime<Utc>,
    pub seating_count: i32,
}

#[derive(Copy, Clone, Debug)]
struct EventDetails {
    event_type: EventType,
    occurred_at: DateTime<Utc>,
}

#[tracing::instrument(skip(pool))]
pub async fn get_daily_stats(
    pool: &PgPool,
    today_start: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Result<DailyStats> {
    // Get state just before today (to know if retainer was in/out at midnight)
    let previous_event = sqlx::query_as!(
        EventDetails,
        r#"
            SELECT ret.name AS "event_type: EventType", re.occurred_at
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
    let mut today_events = sqlx::query_as!(
        EventDetails,
        r#"
            SELECT ret.name AS "event_type: EventType", re.occurred_at
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
    let default_state = match previous_event.map(|e| e.event_type) {
        Some(EventType::Inserted) => EventType::Inserted,
        Some(EventType::Removed) | None => EventType::Removed,
    };

    // insert a synthetic event at midnight to simplify wear time calculation logic
    let precursor_event = previous_event.unwrap_or(EventDetails {
        event_type: default_state,
        occurred_at: today_start,
    });

    today_events.insert(0, precursor_event);

    let (current_state, latest_event_time, wear_minutes, out_minutes) =
        if let Some(last_event) = today_events.last() {
            let (mut wear_minutes, mut out_minutes) = today_events.iter().tuple_windows().fold(
                (0, 0),
                |(wear_acc, out_acc), (before, after)| {
                    let duration = after
                        .occurred_at
                        .signed_duration_since(std::cmp::max(before.occurred_at, today_start))
                        .num_minutes();

                    match before.event_type {
                        EventType::Inserted => (wear_acc + duration, out_acc),
                        EventType::Removed => (wear_acc, out_acc + duration),
                    }
                },
            );

            if last_event.event_type == EventType::Inserted {
                wear_minutes += now
                    .signed_duration_since(std::cmp::max(last_event.occurred_at, today_start))
                    .num_minutes();
            } else {
                out_minutes += now
                    .signed_duration_since(std::cmp::max(last_event.occurred_at, today_start))
                    .num_minutes();
            }

            (
                last_event.event_type,
                last_event.occurred_at,
                wear_minutes,
                out_minutes,
            )
        } else {
            // No events at all today, state is whatever it was at midnight
            let elapsed = now.signed_duration_since(today_start).num_minutes();

            let (wear_minutes, out_minutes) = match default_state {
                EventType::Inserted => (elapsed, 0),
                EventType::Removed => (0, elapsed),
            };

            (default_state, today_start, wear_minutes, out_minutes)
        };

    let is_on_track = out_minutes < 2 * 60;

    let seating_count = sqlx::query_scalar!(
        r#"SELECT COUNT(*)::INT FROM retainer_seating WHERE occurred_at >= $1 AND occurred_at < $2"#,
        today_start,
        now,
    )
    .fetch_one(pool)
    .await?
    .unwrap_or(0);

    Ok(DailyStats {
        wear_minutes,
        out_minutes,
        is_on_track,
        current_state,
        latest_event_time,
        seating_count,
    })
}

pub struct DayHistory {
    pub date: NaiveDate,
    pub wear_minutes: i64,
    pub out_minutes: i64,
    pub is_on_track: bool,
    pub seating_count: i32,
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

    let all_seatings =
        sqlx::query_scalar!(r#"SELECT occurred_at FROM retainer_seating ORDER BY occurred_at ASC"#)
            .fetch_all(pool)
            .await?;

    let first_date = all_events[0].occurred_at.date_naive();
    let today_date = now.date_naive();

    let mut results = Vec::new();
    let mut carry_inserted = false;
    let mut event_idx = 0;
    let mut seating_idx = 0;
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

        let mut seating_count: i32 = 0;
        while seating_idx < all_seatings.len() && all_seatings[seating_idx] < effective_end {
            if all_seatings[seating_idx] >= day_start {
                seating_count += 1;
            }
            seating_idx += 1;
        }

        results.push(DayHistory {
            date: day,
            wear_minutes,
            out_minutes,
            is_on_track,
            seating_count,
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

#[tracing::instrument(skip(pool))]
pub async fn record_seating(pool: &PgPool, occurred_at: DateTime<Utc>) -> Result<()> {
    sqlx::query!(
        r#"INSERT INTO retainer_seating (occurred_at) VALUES ($1)"#,
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

    use super::{EventType, get_daily_stats, get_history, record_event, record_seating};

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
    async fn most_recent_event_time_is_accurate(pool: PgPool) -> Result<()> {
        let inserted_at = today_start() + chrono::Duration::hours(9);
        let removed_at = today_start() + chrono::Duration::hours(10);

        record_event(&pool, EventType::Inserted, inserted_at).await?;
        record_event(&pool, EventType::Removed, removed_at).await?;

        let now = today_start() + chrono::Duration::hours(11);
        let stats = get_daily_stats(&pool, today_start(), now).await?;

        assert_eq!(stats.current_state, EventType::Removed);
        assert_eq!(stats.latest_event_time, removed_at);

        Ok(())
    }

    #[sqlx::test]
    async fn most_recent_event_time_includes_previous_days_if_required(pool: PgPool) -> Result<()> {
        let removed_at = today_start() - chrono::Duration::hours(3);
        record_event(&pool, EventType::Removed, removed_at).await?;

        let now = today_start() + chrono::Duration::hours(10);
        let stats = get_daily_stats(&pool, today_start(), now).await?;

        assert_eq!(stats.current_state, EventType::Removed);
        assert_eq!(stats.latest_event_time, removed_at);

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

    // --- seating tests ---

    #[sqlx::test]
    async fn no_seatings_gives_zero_count(pool: PgPool) -> Result<()> {
        let now = today_start() + chrono::Duration::hours(10);
        let stats = get_daily_stats(&pool, today_start(), now).await?;

        assert_eq!(stats.seating_count, 0);

        Ok(())
    }

    #[sqlx::test]
    async fn recording_seatings_increments_count(pool: PgPool) -> Result<()> {
        let t = today_start() + chrono::Duration::hours(8);
        record_seating(&pool, t).await?;
        record_seating(&pool, t + chrono::Duration::hours(4)).await?;

        let now = today_start() + chrono::Duration::hours(14);
        let stats = get_daily_stats(&pool, today_start(), now).await?;

        assert_eq!(stats.seating_count, 2);

        Ok(())
    }

    #[sqlx::test]
    async fn seating_from_previous_day_not_counted_today(pool: PgPool) -> Result<()> {
        // Seating yesterday should not appear in today's count
        record_seating(&pool, today_start() - chrono::Duration::hours(1)).await?;

        let now = today_start() + chrono::Duration::hours(10);
        let stats = get_daily_stats(&pool, today_start(), now).await?;

        assert_eq!(stats.seating_count, 0);

        Ok(())
    }

    #[sqlx::test]
    async fn history_includes_correct_seating_count_per_day(pool: PgPool) -> Result<()> {
        let day1 = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let day2 = Utc.with_ymd_and_hms(2026, 1, 2, 0, 0, 0).unwrap();

        // Anchor events so history spans two days
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

        // Two seatings on day 1, one on day 2
        record_seating(&pool, day1 + chrono::Duration::hours(9)).await?;
        record_seating(&pool, day1 + chrono::Duration::hours(18)).await?;
        record_seating(&pool, day2 + chrono::Duration::hours(9)).await?;

        let now = Utc.with_ymd_and_hms(2026, 1, 2, 12, 0, 0).unwrap();
        let history = get_history(&pool, now).await?;

        // history[0] = Jan 2, history[1] = Jan 1
        assert_eq!(history[1].seating_count, 2);
        assert_eq!(history[0].seating_count, 1);

        Ok(())
    }
}
