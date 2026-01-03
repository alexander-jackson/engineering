use chrono::{NaiveDate, NaiveTime};
use sqlx::PgPool;
use uuid::Uuid;

use opentracker::persistence;

pub const SOME_EMAIL: &str = "example@email.com";
pub const SOME_EQUIVALENT_EMAIL: &str = "eXaMplE@EmaIl.com";
pub const SOME_HASHED_PASSWORD: &str = "<hashed>";

pub async fn some_user(pool: &PgPool) -> sqlx::Result<Uuid> {
    persistence::account::insert(SOME_EMAIL, SOME_HASHED_PASSWORD, pool).await
}

pub fn date(day: u32, month: u32, year: i32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).unwrap()
}

pub fn time(hour: u32, min: u32, sec: u32) -> NaiveTime {
    NaiveTime::from_hms_opt(hour, min, sec).unwrap()
}
