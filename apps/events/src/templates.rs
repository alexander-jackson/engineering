use serde::Serialize;

use crate::persistence::{DailyStats, DayHistory, EventType};

#[derive(Serialize)]
pub struct IndexContext {
    pub is_inserted: bool,
    pub wear_time_display: String,
    pub out_time_display: String,
    pub target_display: String,
    pub is_on_track: bool,
    pub action_label: &'static str,
    pub action_path: &'static str,
}

impl From<DailyStats> for IndexContext {
    fn from(stats: DailyStats) -> Self {
        let is_inserted = stats.current_state == EventType::Inserted;

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
            wear_time_display,
            out_time_display,
            target_display,
            is_on_track: stats.is_on_track,
            action_label,
            action_path,
        }
    }
}

#[derive(Serialize)]
pub struct HistoryEntry {
    pub date: String,
    pub wear_time_display: String,
    pub out_time_display: String,
    pub is_on_track: bool,
}

#[derive(Serialize)]
pub struct HistoryContext {
    pub entries: Vec<HistoryEntry>,
}

impl From<Vec<DayHistory>> for HistoryContext {
    fn from(days: Vec<DayHistory>) -> Self {
        let entries = days
            .into_iter()
            .map(|day| HistoryEntry {
                date: day.date.format("%a, %d %b %Y").to_string(),
                wear_time_display: format_minutes(day.wear_minutes),
                out_time_display: format_minutes(day.out_minutes),
                is_on_track: day.is_on_track,
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
