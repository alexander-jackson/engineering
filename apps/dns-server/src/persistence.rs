use std::collections::{HashMap, HashSet};
use std::ops::DerefMut;

use color_eyre::eyre::Result;
use sqlx::types::{Uuid, chrono::Utc};
use sqlx::{Postgres, Transaction};

use crate::classifier::{Prompt, Threshold};
use crate::uid::QuestionUid;

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

#[derive(Copy, Clone, Debug)]
pub enum QuestionEventType {
    /// The question was removed by a user.
    Retired,
    /// The question was superseded by a newer one.
    Redundant,
}

impl QuestionEventType {
    fn as_str(&self) -> &'static str {
        match self {
            QuestionEventType::Retired => "Retired",
            QuestionEventType::Redundant => "Redundant",
        }
    }
}

pub async fn insert_domain(tx: &mut Transaction<'_, Postgres>, domain: &str) -> Result<Uuid> {
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
    tx: &mut Transaction<'_, Postgres>,
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

pub struct QuestionRow {
    pub classifier_question_uid: Uuid,
    pub prompt: String,
    pub threshold: f64,
}

/// Select the questions that are currently asked of new domains, oldest first.
pub async fn select_active_questions(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<Vec<QuestionRow>> {
    let rows = sqlx::query_as!(
        QuestionRow,
        r#"
            SELECT q.classifier_question_uid, q.prompt, q.threshold
            FROM classifier_question q
            WHERE NOT EXISTS (
                SELECT 1
                FROM classifier_question_event e
                WHERE e.classifier_question_id = q.id
            )
            ORDER BY q.created_at
        "#
    )
    .fetch_all(tx.deref_mut())
    .await?;

    Ok(rows)
}

pub async fn insert_question(
    tx: &mut Transaction<'_, Postgres>,
    prompt: &Prompt,
    threshold: Threshold,
) -> Result<QuestionUid> {
    let classifier_question_uid = QuestionUid::new();
    let now = Utc::now();

    sqlx::query!(
        r#"
            INSERT INTO classifier_question (classifier_question_uid, prompt, threshold, created_at)
            VALUES ($1, $2, $3, $4)
        "#,
        *classifier_question_uid,
        &**prompt,
        *threshold,
        now
    )
    .execute(tx.deref_mut())
    .await?;

    Ok(classifier_question_uid)
}

/// Mark an active question as no longer asked, returning whether an active question was found.
pub async fn end_question(
    tx: &mut Transaction<'_, Postgres>,
    classifier_question_uid: QuestionUid,
    event_type: QuestionEventType,
) -> Result<bool> {
    let classifier_question_event_uid = Uuid::new_v4();
    let now = Utc::now();

    let query_result = sqlx::query!(
        r#"
            INSERT INTO classifier_question_event (
                classifier_question_event_uid,
                classifier_question_id,
                event_type_id,
                created_at
            )
            SELECT
                $1,
                q.id,
                (SELECT id FROM classifier_question_event_type WHERE name = $3),
                $4
            FROM classifier_question q
            WHERE q.classifier_question_uid = $2
            ON CONFLICT (classifier_question_id) DO NOTHING
        "#,
        classifier_question_event_uid,
        *classifier_question_uid,
        event_type.as_str(),
        now
    )
    .execute(tx.deref_mut())
    .await?;

    Ok(query_result.rows_affected() == 1)
}

pub async fn select_domain_decisions(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<HashMap<String, bool>> {
    let rows = sqlx::query!(
        r#"
            SELECT name, blocked
            FROM domain_decision
        "#
    )
    .fetch_all(tx.deref_mut())
    .await?;

    Ok(rows.into_iter().map(|r| (r.name, r.blocked)).collect())
}

/// Record the decision for a domain alongside the score each question gave it.
///
/// A domain is only decided once, so returns `false` and records nothing if a decision already
/// exists for it.
pub async fn insert_domain_decision(
    tx: &mut Transaction<'_, Postgres>,
    domain: &str,
    blocked: bool,
    scores: &[(QuestionUid, f64)],
) -> Result<bool> {
    let domain_decision_uid = Uuid::new_v4();
    let now = Utc::now();

    let domain_decision_id = sqlx::query_scalar!(
        r#"
            INSERT INTO domain_decision (domain_decision_uid, name, blocked, decided_at)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (name) DO NOTHING
            RETURNING id
        "#,
        domain_decision_uid,
        domain,
        blocked,
        now
    )
    .fetch_optional(tx.deref_mut())
    .await?;

    let Some(domain_decision_id) = domain_decision_id else {
        return Ok(false);
    };

    for (classifier_question_uid, score) in scores {
        sqlx::query!(
            r#"
                INSERT INTO domain_decision_question (
                    domain_decision_id,
                    classifier_question_id,
                    score
                )
                VALUES (
                    $1,
                    (SELECT id FROM classifier_question WHERE classifier_question_uid = $2),
                    $3
                )
            "#,
            domain_decision_id,
            **classifier_question_uid,
            score
        )
        .execute(tx.deref_mut())
        .await?;
    }

    Ok(true)
}

#[cfg(test)]
mod tests {
    use color_eyre::eyre::Result;
    use sqlx::PgPool;

    use crate::classifier::{Prompt, Threshold};
    use crate::uid::QuestionUid;

    use crate::persistence::{
        DomainEventType, QuestionEventType, end_question, insert_domain, insert_domain_decision,
        insert_domain_event, insert_question, select_active_questions, select_blocked_domains,
        select_domain_decisions,
    };

    async fn add_question(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        prompt: &str,
        threshold: f64,
    ) -> Result<QuestionUid> {
        let prompt = Prompt::try_from(prompt)?;
        let threshold = Threshold::try_from(threshold)?;

        insert_question(tx, &prompt, threshold).await
    }

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

    #[sqlx::test]
    async fn decisions_are_selected(pool: PgPool) -> Result<()> {
        let mut tx = pool.begin().await?;
        assert!(insert_domain_decision(&mut tx, "example.com", false, &[]).await?);
        assert!(insert_domain_decision(&mut tx, "bad.example.com", true, &[]).await?);
        tx.commit().await?;

        let mut tx = pool.begin().await?;
        let decisions = select_domain_decisions(&mut tx).await?;

        assert_eq!(decisions.len(), 2);
        assert!(!decisions["example.com"]);
        assert!(decisions["bad.example.com"]);

        Ok(())
    }

    #[sqlx::test]
    async fn decisions_record_the_scores_of_their_questions(pool: PgPool) -> Result<()> {
        let mut tx = pool.begin().await?;
        let proxy = add_question(&mut tx, "proxy?", 0.8).await?;
        let adult = add_question(&mut tx, "adult?", 0.8).await?;
        let scores = [(proxy, 0.95), (adult, 0.1)];

        assert!(insert_domain_decision(&mut tx, "bad.example.com", true, &scores).await?);
        tx.commit().await?;

        let rows = sqlx::query!(
            r#"
                SELECT q.classifier_question_uid, ddq.score
                FROM domain_decision_question ddq
                JOIN domain_decision dd ON dd.id = ddq.domain_decision_id
                JOIN classifier_question q ON q.id = ddq.classifier_question_id
                WHERE dd.name = 'bad.example.com'
                ORDER BY q.created_at
            "#
        )
        .fetch_all(&pool)
        .await?;

        let actual: Vec<_> = rows
            .into_iter()
            .map(|r| (QuestionUid::from(r.classifier_question_uid), r.score))
            .collect();
        assert_eq!(actual, scores);

        Ok(())
    }

    #[sqlx::test]
    async fn a_domain_is_only_decided_once(pool: PgPool) -> Result<()> {
        let mut tx = pool.begin().await?;
        let question = add_question(&mut tx, "question", 0.5).await?;

        assert!(insert_domain_decision(&mut tx, "example.com", false, &[(question, 0.1)]).await?);
        assert!(!insert_domain_decision(&mut tx, "example.com", true, &[(question, 0.9)]).await?);
        tx.commit().await?;

        let mut tx = pool.begin().await?;
        let decisions = select_domain_decisions(&mut tx).await?;
        assert!(!decisions["example.com"]);

        let scores = sqlx::query_scalar!("SELECT score FROM domain_decision_question")
            .fetch_all(&pool)
            .await?;
        assert_eq!(scores, vec![0.1]);

        Ok(())
    }

    #[sqlx::test]
    async fn active_questions_are_selected_oldest_first(pool: PgPool) -> Result<()> {
        let mut tx = pool.begin().await?;
        let first = add_question(&mut tx, "first", 0.7).await?;
        let retired = add_question(&mut tx, "retired", 0.1).await?;
        let redundant = add_question(&mut tx, "redundant", 0.1).await?;
        let second = add_question(&mut tx, "second", 0.5).await?;
        assert!(end_question(&mut tx, retired, QuestionEventType::Retired).await?);
        assert!(end_question(&mut tx, redundant, QuestionEventType::Redundant).await?);
        tx.commit().await?;

        let mut tx = pool.begin().await?;
        let questions = select_active_questions(&mut tx).await?;

        let uids: Vec<_> = questions
            .iter()
            .map(|q| QuestionUid::from(q.classifier_question_uid))
            .collect();
        assert_eq!(uids, vec![first, second]);
        assert_eq!(questions[0].prompt, "first");
        assert_eq!(questions[0].threshold, 0.7);

        Ok(())
    }

    #[sqlx::test]
    async fn questions_can_only_be_ended_once(pool: PgPool) -> Result<()> {
        let mut tx = pool.begin().await?;
        let uid = add_question(&mut tx, "question", 0.5).await?;

        assert!(end_question(&mut tx, uid, QuestionEventType::Retired).await?);
        assert!(!end_question(&mut tx, uid, QuestionEventType::Redundant).await?);
        assert!(!end_question(&mut tx, QuestionUid::new(), QuestionEventType::Retired).await?);

        Ok(())
    }

    #[sqlx::test]
    async fn questions_and_their_events_are_append_only(pool: PgPool) -> Result<()> {
        let mut tx = pool.begin().await?;
        let uid = add_question(&mut tx, "question", 0.5).await?;
        end_question(&mut tx, uid, QuestionEventType::Retired).await?;
        tx.commit().await?;

        for statement in [
            "UPDATE classifier_question SET prompt = 'changed'",
            "UPDATE classifier_question SET threshold = 0.1",
            "DELETE FROM classifier_question",
            "UPDATE classifier_question_event SET event_type_id = event_type_id + 1",
            "DELETE FROM classifier_question_event",
        ] {
            let result = sqlx::query(statement).execute(&pool).await;

            assert!(result.is_err(), "{statement} should have been rejected");
        }

        Ok(())
    }
}
