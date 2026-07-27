use std::collections::HashSet;
use std::ops::DerefMut;

use color_eyre::eyre::Result;
use sqlx::types::{Uuid, chrono::Utc};
use sqlx::{Postgres, Transaction};

#[derive(Copy, Clone, Debug, sqlx::Type)]
pub enum DomainEventType {
    Blocked,
}

impl DomainEventType {
    fn as_str(&self) -> &'static str {
        match self {
            DomainEventType::Blocked => "Blocked",
        }
    }
}

pub async fn insert_domain(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    domain: &str,
) -> Result<Uuid> {
    let domain_uid = Uuid::new_v4();

    let query_result = sqlx::query!(
        r#"
            INSERT INTO domain (domain_uid, name)
            VALUES ($1, $2)
            ON CONFLICT (name) DO NOTHING
        "#,
        domain_uid,
        domain,
    )
    .execute(tx.deref_mut())
    .await?;

    if query_result.rows_affected() == 0 {
        // Domain already exists, fetch its UID
        let existing_domain = sqlx::query_scalar!(
            r#"
                SELECT domain_uid
                FROM domain
                WHERE name = $1
            "#,
            domain,
        )
        .fetch_one(tx.deref_mut())
        .await?;

        return Ok(existing_domain);
    }

    Ok(domain_uid)
}

pub async fn insert_domain_event(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    domain_uid: Uuid,
    event_type: DomainEventType,
) -> Result<Uuid> {
    let domain_event_uid = Uuid::new_v4();
    let now = Utc::now();

    sqlx::query!(
        r#"
            INSERT INTO domain_event (domain_event_uid, domain_id, event_type_id, created_at)
            VALUES (
                $1,
                (SELECT id FROM domain WHERE domain_uid = $2),
                (SELECT id FROM domain_event_type WHERE name = $3),
                $4
            )
        "#,
        domain_event_uid,
        domain_uid,
        event_type.as_str(),
        now
    )
    .execute(tx.deref_mut())
    .await?;

    Ok(domain_event_uid)
}

pub async fn select_blocked_domains(tx: &mut Transaction<'_, Postgres>) -> Result<HashSet<String>> {
    let rows: Vec<String> = sqlx::query_scalar!(
        r#"
            SELECT d.name
            FROM domain d
            JOIN LATERAL (
                SELECT det.name
                FROM domain_event de
                JOIN domain_event_type det ON de.event_type_id = det.id
                WHERE de.domain_id = d.id
                ORDER BY de.created_at DESC
                LIMIT 1
            ) latest_event ON true
            WHERE latest_event.name = 'Blocked'
        "#
    )
    .fetch_all(tx.deref_mut())
    .await?;

    Ok(rows.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use color_eyre::eyre::Result;
    use sqlx::PgPool;

    use crate::persistence::{
        DomainEventType, insert_domain, insert_domain_event, select_blocked_domains,
    };

    #[sqlx::test]
    async fn can_block_domains_and_select_them(pool: PgPool) -> Result<()> {
        let domain = "example.com";
        let mut tx = pool.begin().await?;

        // Insert a blocked domain for testing
        let domain_uid = insert_domain(&mut tx, domain).await?;

        // Insert a block event for the domain
        insert_domain_event(&mut tx, domain_uid, DomainEventType::Blocked).await?;

        // Commit the transaction
        tx.commit().await?;

        // Start a new transaction to select blocked domains
        let mut tx = pool.begin().await?;
        let blocked_domains = select_blocked_domains(&mut tx).await?;

        assert!(blocked_domains.contains(domain));

        Ok(())
    }

    #[sqlx::test]
    async fn domain_insertion_is_handled_idempotently(pool: PgPool) -> Result<()> {
        let domain = "example.com";
        let mut tx = pool.begin().await?;

        // Insert the domain for the first time
        let domain_uid1 = insert_domain(&mut tx, domain).await?;

        // Attempt to insert the same domain again
        let domain_uid2 = insert_domain(&mut tx, domain).await?;

        // Commit the transaction
        tx.commit().await?;

        // The UIDs should be the same since the domain already exists
        assert_eq!(domain_uid1, domain_uid2);

        Ok(())
    }
}
