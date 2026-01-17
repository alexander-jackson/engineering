use chrono::NaiveDateTime;
use color_eyre::eyre::{Result, eyre};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize)]
pub enum BagType {
    #[serde(rename = "PeakDesign30L")]
    PeakDesign30L,
    #[serde(rename = "StubbleAndCo20L")]
    StubbleAndCo20L,
}

impl BagType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PeakDesign30L => "PeakDesign30L",
            Self::StubbleAndCo20L => "StubbleAndCo20L",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::PeakDesign30L => "Peak Design 30L",
            Self::StubbleAndCo20L => "Stubble & Co 20L",
        }
    }

    pub fn icon_path(&self) -> &'static str {
        match self {
            Self::PeakDesign30L => "/assets/icons/peak-design.svg",
            Self::StubbleAndCo20L => "/assets/icons/stubble-co.svg",
        }
    }
}

impl From<String> for BagType {
    fn from(value: String) -> Self {
        match value.as_str() {
            "PeakDesign30L" => Self::PeakDesign30L,
            "StubbleAndCo20L" => Self::StubbleAndCo20L,
            _ => panic!("invalid bag type: {value}"),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Locker {
    pub locker_uid: Uuid,
    pub locker_number: i16,
    pub bag_type: BagType,
    pub checked_in_at: NaiveDateTime,
}

/// Query all currently occupied lockers by finding the latest event for each locker
/// and filtering to only those where the latest event is a CHECK_IN
#[tracing::instrument(skip(pool))]
pub async fn select_all_lockers(pool: &PgPool) -> Result<Vec<Locker>> {
    let lockers = sqlx::query_as!(
        Locker,
        r#"
            WITH latest_events AS (
                SELECT DISTINCT ON (locker_number)
                    le.locker_event_uid,
                    le.locker_number,
                    le.bag_type_id,
                    le.locker_event_type_id,
                    le.occurred_at
                FROM locker_event le
                ORDER BY le.locker_number, le.occurred_at DESC
            )
            SELECT
                latest.locker_event_uid as locker_uid,
                latest.locker_number,
                bt.name as "bag_type!: String",
                latest.occurred_at as checked_in_at
            FROM latest_events latest
            JOIN bag_type bt ON bt.id = latest.bag_type_id
            JOIN locker_event_type let ON let.id = latest.locker_event_type_id
            WHERE let.name = 'CheckIn'
            ORDER BY latest.locker_number ASC
        "#
    )
    .fetch_all(pool)
    .await?;

    Ok(lockers)
}

/// Insert a CHECK_IN event for a bag being placed in a locker
#[tracing::instrument(skip(pool))]
pub async fn insert_check_in_event(
    pool: &PgPool,
    event_uid: Uuid,
    locker_number: i16,
    bag_type: BagType,
    occurred_at: NaiveDateTime,
) -> Result<()> {
    // First check if locker is already occupied
    if is_locker_occupied(pool, locker_number).await? {
        return Err(eyre!("Locker #{} is already occupied", locker_number));
    }

    // Check if bag is already checked in elsewhere
    if is_bag_checked_in(pool, bag_type).await? {
        return Err(eyre!(
            "{} is already checked in to another locker",
            bag_type.display_name()
        ));
    }

    sqlx::query!(
        r#"
            INSERT INTO locker_event (locker_event_uid, locker_number, bag_type_id, locker_event_type_id, occurred_at)
            VALUES (
                $1,
                $2,
                (SELECT id FROM bag_type WHERE name = $3),
                (SELECT id FROM locker_event_type WHERE name = 'CheckIn'),
                $4
            )
        "#,
        event_uid,
        locker_number,
        bag_type.as_str(),
        occurred_at,
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Insert a CHECK_OUT event for a bag being removed from a locker
#[tracing::instrument(skip(pool))]
pub async fn insert_check_out_event(
    pool: &PgPool,
    event_uid: Uuid,
    locker_number: i16,
    occurred_at: NaiveDateTime,
) -> Result<()> {
    // Verify locker is currently occupied
    if !is_locker_occupied(pool, locker_number).await? {
        return Err(eyre!("Locker #{} is not occupied", locker_number));
    }

    // Get the bag type from the latest CHECK_IN event
    let bag_type_id = sqlx::query_scalar!(
        r#"
            SELECT DISTINCT ON (locker_number) bag_type_id
            FROM locker_event
            WHERE locker_number = $1
            ORDER BY locker_number, occurred_at DESC
        "#,
        locker_number
    )
    .fetch_one(pool)
    .await?;

    sqlx::query!(
        r#"
            INSERT INTO locker_event (locker_event_uid, locker_number, bag_type_id, locker_event_type_id, occurred_at)
            VALUES (
                $1,
                $2,
                $3,
                (SELECT id FROM locker_event_type WHERE name = 'CheckOut'),
                $4
            )
        "#,
        event_uid,
        locker_number,
        bag_type_id,
        occurred_at,
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Check if a locker is currently occupied (latest event is CHECK_IN)
async fn is_locker_occupied(pool: &PgPool, locker_number: i16) -> Result<bool> {
    let result = sqlx::query_scalar!(
        r#"
            SELECT let.name
            FROM (
                SELECT DISTINCT ON (locker_number) locker_event_type_id
                FROM locker_event
                WHERE locker_number = $1
                ORDER BY locker_number, occurred_at DESC
            ) latest
            JOIN locker_event_type let ON let.id = latest.locker_event_type_id
        "#,
        locker_number
    )
    .fetch_optional(pool)
    .await?;

    Ok(result.as_deref() == Some("CheckIn"))
}

/// Check if a bag is currently checked in anywhere (has CHECK_IN with no subsequent CHECK_OUT)
async fn is_bag_checked_in(pool: &PgPool, bag_type: BagType) -> Result<bool> {
    let result = sqlx::query_scalar!(
        r#"
            SELECT let.name
            FROM (
                SELECT DISTINCT ON (bag_type_id) locker_event_type_id
                FROM locker_event
                WHERE bag_type_id = (SELECT id FROM bag_type WHERE name = $1)
                ORDER BY bag_type_id, occurred_at DESC
            ) latest
            JOIN locker_event_type let ON let.id = latest.locker_event_type_id
        "#,
        bag_type.as_str()
    )
    .fetch_optional(pool)
    .await?;

    Ok(result.as_deref() == Some("CheckIn"))
}
