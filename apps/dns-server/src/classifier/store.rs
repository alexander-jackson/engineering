use std::collections::HashMap;

use color_eyre::eyre::{Result, WrapErr};
use sqlx::PgPool;

use super::{DecisionStore, Prompt, Question, Threshold};
use crate::persistence::QuestionEventType;
use crate::uid::QuestionUid;

#[derive(Clone)]
pub struct PostgresDecisionStore {
    pool: PgPool,
}

impl PostgresDecisionStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl DecisionStore for PostgresDecisionStore {
    async fn questions(&self) -> Result<Vec<Question>> {
        let mut tx = self.pool.begin().await?;
        let rows = crate::persistence::select_active_questions(&mut tx).await?;
        tx.commit().await?;

        rows.into_iter()
            .map(|row| {
                let uid = QuestionUid::from(row.classifier_question_uid);

                Ok(Question {
                    uid,
                    prompt: Prompt::try_from(row.prompt)
                        .wrap_err_with(|| format!("invalid prompt stored for question {uid}"))?,
                    threshold: Threshold::try_from(row.threshold)
                        .wrap_err_with(|| format!("invalid threshold stored for question {uid}"))?,
                })
            })
            .collect()
    }

    async fn decisions(&self) -> Result<HashMap<String, bool>> {
        let mut tx = self.pool.begin().await?;
        let decisions = crate::persistence::select_domain_decisions(&mut tx).await?;
        tx.commit().await?;

        Ok(decisions)
    }

    async fn record(
        &self,
        domain: &str,
        blocked: bool,
        scores: &[(QuestionUid, f64)],
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        crate::persistence::insert_domain_decision(&mut tx, domain, blocked, scores).await?;
        tx.commit().await?;

        Ok(())
    }

    async fn add_question(&self, prompt: &Prompt, threshold: Threshold) -> Result<QuestionUid> {
        let mut tx = self.pool.begin().await?;
        let uid = crate::persistence::insert_question(&mut tx, prompt, threshold).await?;
        tx.commit().await?;

        Ok(uid)
    }

    async fn retire_question(&self, uid: QuestionUid) -> Result<bool> {
        let mut tx = self.pool.begin().await?;
        let retired =
            crate::persistence::end_question(&mut tx, uid, QuestionEventType::Retired).await?;
        tx.commit().await?;

        Ok(retired)
    }

    async fn replace_question(
        &self,
        uid: QuestionUid,
        prompt: &Prompt,
        threshold: Threshold,
    ) -> Result<Option<QuestionUid>> {
        let mut tx = self.pool.begin().await?;

        if !crate::persistence::end_question(&mut tx, uid, QuestionEventType::Redundant).await? {
            return Ok(None);
        }

        let replacement = crate::persistence::insert_question(&mut tx, prompt, threshold).await?;
        tx.commit().await?;

        Ok(Some(replacement))
    }
}
