use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;

use crate::persistence::{DailyStats, DayHistory, EventType};

#[derive(Serialize)]
pub struct IndexContext {
    pub is_inserted: bool,
    pub latest_event_time: DateTime<Utc>,
    pub wear_time_display: String,
    pub out_time_display: String,
    pub target_display: String,
    pub is_on_track: bool,
    pub action_label: &'static str,
    pub action_path: &'static str,
    pub seating_count: i32,
    pub seating_target: i32,
}

impl From<DailyStats> for IndexContext {
    fn from(stats: DailyStats) -> Self {
        let is_inserted = stats.current_state == EventType::Inserted;
        let latest_event_time = stats.latest_event_time;

        let wear_time_display = format_minutes(stats.wear_minutes);
        let out_time_display = format_minutes(stats.out_minutes);

        let budget_minutes = 2 * 60;
        let remaining_budget = (budget_minutes - stats.out_minutes).max(0);
        let target_display = if stats.out_minutes >= budget_minutes {
            "Out-time budget exceeded".to_owned()
        } else {
            format!(
                "{} of out-time budget remaining",
                format_minutes(remaining_budget)
            )
        };

        let (action_label, action_path) = if is_inserted {
            ("Take Out", "/remove")
        } else {
            ("Put In", "/insert")
        };

        Self {
            is_inserted,
            latest_event_time,
            wear_time_display,
            out_time_display,
            target_display,
            is_on_track: stats.is_on_track,
            action_label,
            action_path,
            seating_count: stats.seating_count,
            seating_target: 2,
        }
    }
}

#[derive(Serialize)]
pub struct HistoryEntry {
    pub date: String,
    pub wear_time_display: String,
    pub out_time_display: String,
    pub is_on_track: bool,
    pub seatings: Option<SeatingInformation>,
}

#[derive(Serialize)]
pub struct SeatingInformation {
    pub count: i32,
    pub target: i32,
}

#[derive(Serialize)]
pub struct HistoryContext {
    pub entries: Vec<HistoryEntry>,
}

impl HistoryContext {
    pub fn from(days: Vec<DayHistory>, cutoff: NaiveDate) -> Self {
        let entries = days
            .into_iter()
            .map(|day| {
                let seatings = if day.date >= cutoff {
                    Some(SeatingInformation {
                        count: day.seating_count,
                        target: 2,
                    })
                } else {
                    None
                };

                HistoryEntry {
                    date: day.date.format("%a, %d %b %Y").to_string(),
                    wear_time_display: format_minutes(day.wear_minutes),
                    out_time_display: format_minutes(day.out_minutes),
                    is_on_track: day.is_on_track,
                    seatings,
                }
            })
            .collect();

        Self { entries }
    }
}

fn format_minutes(minutes: i64) -> String {
    let hours = minutes / 60;
    let mins = minutes % 60;
    if hours > 0 {
        format!("{}h {}m", hours, mins)
    } else {
        format!("{}m", mins)
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use crate::persistence::DayHistory;
    use crate::templates::HistoryContext;

    #[test]
    fn cutoff_applies_to_history_entries() {
        let cutoff = NaiveDate::from_ymd_opt(2024, 6, 1).unwrap();

        let context = HistoryContext::from(
            vec![
                DayHistory {
                    date: NaiveDate::from_ymd_opt(2024, 6, 2).unwrap(),
                    wear_minutes: 480,
                    out_minutes: 120,
                    is_on_track: true,
                    seating_count: 1,
                },
                DayHistory {
                    date: NaiveDate::from_ymd_opt(2024, 5, 31).unwrap(),
                    wear_minutes: 450,
                    out_minutes: 150,
                    is_on_track: false,
                    seating_count: 0,
                },
            ],
            cutoff,
        );

        assert!(context.entries[0].seatings.is_some());
        assert!(context.entries[1].seatings.is_none());
    }
}
