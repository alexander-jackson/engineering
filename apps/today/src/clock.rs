use chrono::{Days, NaiveDate, Utc};
use chrono_tz::Europe::London;

/// The day the list belongs to, judged by the London calendar
pub fn today() -> NaiveDate {
    Utc::now().with_timezone(&London).date_naive()
}

/// How many days before today can still be viewed
pub const HISTORY_DAYS: u64 = 7;

/// The earliest day that can be viewed, given what today is
pub fn earliest_viewable(today: NaiveDate) -> NaiveDate {
    today - Days::new(HISTORY_DAYS)
}
