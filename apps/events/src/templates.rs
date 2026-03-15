use serde::Serialize;

use crate::persistence::{DailyStats, EventType};

#[derive(Serialize)]
pub struct IndexContext {
    pub is_inserted: bool,
    pub wear_time_display: String,
    pub target_display: String,
    pub is_on_track: bool,
    pub action_label: &'static str,
    pub action_path: &'static str,
}

impl From<DailyStats> for IndexContext {
    fn from(stats: DailyStats) -> Self {
        let is_inserted = stats.current_state == EventType::Inserted;

        let wear_time_display = format_minutes(stats.wear_minutes);

        let target_minutes = 22 * 60;
        let remaining_target = (target_minutes - stats.wear_minutes).max(0);
        let target_display = if remaining_target == 0 {
            "Target achieved!".to_owned()
        } else {
            format!(
                "{} remaining to hit target",
                format_minutes(remaining_target)
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
            target_display,
            is_on_track: stats.is_on_track,
            action_label,
            action_path,
        }
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
