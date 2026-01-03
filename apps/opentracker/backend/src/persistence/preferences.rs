use sqlx::PgPool;
use uuid::Uuid;

#[derive(sqlx::Type)]
#[sqlx(type_name = "rep_set_notation")]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum RepSetNotation {
    SetsThenReps,
    RepsThenSets,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Preferences {
    rep_set_notation: RepSetNotation,
}

impl Preferences {
    pub fn new(rep_set_notation: RepSetNotation) -> Self {
        Self { rep_set_notation }
    }
}

pub async fn fetch(user_id: Uuid, pool: &PgPool) -> sqlx::Result<Option<Preferences>> {
    let preferences = sqlx::query_as!(
        Preferences,
        r#"
        SELECT rep_set_notation AS "rep_set_notation: RepSetNotation"
        FROM user_preference up
        WHERE account_id = (SELECT id FROM account WHERE account_uid = $1)
        "#,
        user_id,
    )
    .fetch_optional(pool)
    .await?;

    Ok(preferences)
}

pub async fn update(user_id: Uuid, preferences: Preferences, pool: &PgPool) -> sqlx::Result<()> {
    sqlx::query!(
        r#"
        INSERT INTO user_preference (account_id, rep_set_notation)
        VALUES ((SELECT id FROM account WHERE account_uid = $1), $2)
        ON CONFLICT ON CONSTRAINT uk_user_preference_account_id DO UPDATE SET
            rep_set_notation = EXCLUDED.rep_set_notation
        "#,
        user_id,
        preferences.rep_set_notation as RepSetNotation,
    )
    .execute(pool)
    .await?;

    Ok(())
}
