use serde::Serialize;

use crate::classifier::Question;

#[derive(Debug, Serialize)]
pub struct IndexContext {
    pub domains: Vec<String>,
}

impl IndexContext {
    pub fn new(domains: impl IntoIterator<Item = String>) -> Self {
        let mut domains: Vec<_> = domains.into_iter().collect();
        domains.sort();

        Self { domains }
    }
}

#[derive(Debug, Serialize)]
pub struct QuestionView {
    pub uid: String,
    pub prompt: String,
    pub threshold: f64,
}

#[derive(Debug, Serialize)]
pub struct QuestionsContext {
    pub questions: Vec<QuestionView>,
}

impl QuestionsContext {
    pub fn new(questions: impl IntoIterator<Item = Question>) -> Self {
        let questions = questions
            .into_iter()
            .map(|question| QuestionView {
                uid: question.uid.to_string(),
                prompt: question.prompt.to_string(),
                threshold: *question.threshold,
            })
            .collect();

        Self { questions }
    }
}
