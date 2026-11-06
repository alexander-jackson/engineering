use chrono::{NaiveDate, Utc};
use chrono_tz::Europe::London;

/// The day the list belongs to, judged by the London calendar
pub fn today() -> NaiveDate {
    Utc::now().with_timezone(&London).date_naive()
}
