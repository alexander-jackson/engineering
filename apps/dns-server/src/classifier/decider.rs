use std::collections::HashMap;

use color_eyre::eyre::{Result, eyre};
use openrouter_rs::OpenRouterClient;
use openrouter_rs::api::decisions::DecisionsRequest;
use serde_json::{Value, json};

use super::{DomainDecider, Question};

/// Scores domains against questions using a decision model (e.g. `typesafe/jev-1.14`) through
/// the OpenRouter Decisions API.
pub struct OpenRouterDecider {
    client: OpenRouterClient,
    model: String,
}

impl OpenRouterDecider {
    pub fn new(api_key: &str, model: &str) -> Result<Self> {
        let client = OpenRouterClient::builder()
            .api_key(api_key)
            .build()
            .map_err(|e| eyre!("failed to build OpenRouter client: {e}"))?;

        Ok(Self {
            client,
            model: model.to_string(),
        })
    }
}

#[async_trait::async_trait]
impl DomainDecider for OpenRouterDecider {
    async fn decide(
        &self,
        domain: &str,
        content: &str,
        questions: &[Question],
    ) -> Result<Vec<f64>> {
        tracing::info!(%domain, "making a decision");

        let request = DecisionsRequest::builder()
            .model(&self.model)
            .state(json!({ "domain": domain, "page_text": content }))
            .questions(build_questions(questions))
            .build()
            .map_err(|e| eyre!("failed to build decisions request: {e}"))?;

        let response = self
            .client
            .decisions()
            .create(&request)
            .await
            .map_err(|e| eyre!("OpenRouter decisions request failed: {e}"))?;

        parse_scores(&response.answers, questions.len())
    }
}

fn question_key(index: usize) -> String {
    format!("q{index}")
}

/// Each question is asked as a yes/no (`noul`) question, which the model answers with the
/// probability that the answer is yes.
fn build_questions(questions: &[Question]) -> Vec<(String, Value)> {
    questions
        .iter()
        .enumerate()
        .map(|(index, question)| {
            let definition = json!({
                "type": "noul",
                "instructions": &*question.prompt,
                "criteria": {
                    "true": "The answer to the question is yes for this website.",
                    "false": "The answer to the question is no for this website.",
                },
            });

            (question_key(index), definition)
        })
        .collect()
}

/// Pull the yes-probability for each question out of the answers, in question order.
fn parse_scores(answers: &HashMap<String, Value>, expected: usize) -> Result<Vec<f64>> {
    (0..expected)
        .map(|index| {
            let key = question_key(index);

            let score = answers
                .get(&key)
                .and_then(|answer| answer.get("noul"))
                .and_then(Value::as_f64)
                .ok_or_else(|| eyre!("no noul score in answer for {key}"))?;

            if !(0.0..=1.0).contains(&score) {
                return Err(eyre!("score for {key} outside of 0.0..=1.0: {score}"));
            }

            Ok(score)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde_json::{Value, json};

    use crate::classifier::{Prompt, Threshold};
    use crate::uid::QuestionUid;

    use super::{Question, build_questions, parse_scores};

    fn question(prompt: &str) -> Question {
        Question {
            uid: QuestionUid::new(),
            prompt: Prompt::try_from(prompt).unwrap(),
            threshold: Threshold::try_from(0.5).unwrap(),
        }
    }

    fn answers(value: Value) -> HashMap<String, Value> {
        serde_json::from_value(value).unwrap()
    }

    #[test]
    fn parses_scores_in_question_order() {
        let answers = answers(json!({
            "q1": { "type": "noul", "noul": 0.9 },
            "q0": { "type": "noul", "noul": 0.1 },
        }));

        assert_eq!(parse_scores(&answers, 2).unwrap(), vec![0.1, 0.9]);
    }

    #[test]
    fn rejects_missing_answers() {
        let answers = answers(json!({ "q0": { "type": "noul", "noul": 0.1 } }));

        assert!(parse_scores(&answers, 2).is_err());
    }

    #[test]
    fn rejects_answers_of_the_wrong_type() {
        let answers = answers(json!({ "q0": { "type": "score", "score": 1.5 } }));

        assert!(parse_scores(&answers, 1).is_err());
    }

    #[test]
    fn rejects_out_of_range_scores() {
        let answers = answers(json!({ "q0": { "type": "noul", "noul": 1.5 } }));

        assert!(parse_scores(&answers, 1).is_err());
    }

    #[test]
    fn builds_a_noul_question_per_prompt() {
        let questions = vec![
            question("Is it gambling?"),
            question("Is it adult content?"),
        ];

        let built = build_questions(&questions);

        assert_eq!(built.len(), 2);
        assert_eq!(built[0].0, "q0");
        assert_eq!(built[1].0, "q1");
        assert_eq!(built[0].1["type"], "noul");
        assert_eq!(built[0].1["instructions"], "Is it gambling?");
        assert_eq!(built[1].1["instructions"], "Is it adult content?");
        assert!(built[0].1["criteria"]["true"].is_string());
        assert!(built[0].1["criteria"]["false"].is_string());
    }
}
