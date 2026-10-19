use opentelemetry::KeyValue;
use opentelemetry::metrics::{Counter, Histogram, Meter};

use crate::classifier::Verdict;

#[derive(Clone)]
pub struct ClassifierMetrics {
    lookups: Counter<u64>,
    jobs: Counter<u64>,
    job_duration: Histogram<f64>,
    fetch_duration: Histogram<f64>,
    decision_duration: Histogram<f64>,
}

impl ClassifierMetrics {
    pub fn new(meter: &Meter) -> Self {
        Self {
            lookups: meter
                .u64_counter("dns_classifier_lookups_total")
                .with_description("Total number of domains passed to the classifier")
                .build(),
            jobs: meter
                .u64_counter("dns_classifier_jobs_total")
                .with_description("Total number of background classification jobs finished")
                .build(),
            job_duration: meter
                .f64_histogram("dns_classifier_job_duration_ms")
                .with_description("Time taken by a whole classification job in milliseconds")
                .build(),
            fetch_duration: meter
                .f64_histogram("dns_classifier_fetch_duration_ms")
                .with_description("Time taken to fetch a domain's content in milliseconds")
                .build(),
            decision_duration: meter
                .f64_histogram("dns_classifier_decision_duration_ms")
                .with_description("Time taken for the decision model to respond in milliseconds")
                .build(),
        }
    }

    /// Record how a call to `classify` was answered.
    pub(crate) fn lookup(&self, result: LookupResult, verdict: Verdict) {
        self.lookups.add(
            1,
            &[
                KeyValue::new("result", result.as_str()),
                KeyValue::new("verdict", verdict.as_str()),
            ],
        );
    }

    /// Record how a background job ended and how long it took.
    pub(crate) fn job(&self, outcome: JobOutcome, verdict: Verdict, elapsed_ms: f64) {
        let attrs = [
            KeyValue::new("outcome", outcome.as_str()),
            KeyValue::new("verdict", verdict.as_str()),
        ];

        self.jobs.add(1, &attrs);
        self.job_duration.record(elapsed_ms, &attrs);
    }

    pub(crate) fn fetch(&self, outcome: FetchOutcome, elapsed_ms: f64) {
        self.fetch_duration
            .record(elapsed_ms, &[KeyValue::new("outcome", outcome.as_str())]);
    }

    pub(crate) fn decision(&self, success: bool, elapsed_ms: f64) {
        let outcome = if success { "success" } else { "error" };

        self.decision_duration
            .record(elapsed_ms, &[KeyValue::new("outcome", outcome)]);
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum LookupResult {
    /// The domain can't have a website, so it was never looked at.
    Unclassifiable,
    /// A stored decision already existed.
    Known,
    /// The domain is new but outside the configured rollout percentage, so it was allowed
    /// without being looked at.
    NotInRollout,
    /// A job finished while the caller was waiting.
    Completed,
    /// The caller gave up waiting and the domain was allowed.
    WaitTimeout,
}

impl LookupResult {
    fn as_str(self) -> &'static str {
        match self {
            Self::Unclassifiable => "unclassifiable",
            Self::Known => "known",
            Self::NotInRollout => "not_in_rollout",
            Self::Completed => "completed",
            Self::WaitTimeout => "wait_timeout",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum JobOutcome {
    /// The decision model scored the content.
    Scored,
    NoQuestions,
    QuestionsFailed,
    NoContent,
    FetchFailed,
    DecisionFailed,
    TimedOut,
}

impl JobOutcome {
    fn as_str(self) -> &'static str {
        match self {
            Self::Scored => "scored",
            Self::NoQuestions => "no_questions",
            Self::QuestionsFailed => "questions_failed",
            Self::NoContent => "no_content",
            Self::FetchFailed => "fetch_failed",
            Self::DecisionFailed => "decision_failed",
            Self::TimedOut => "timed_out",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum FetchOutcome {
    Content,
    Empty,
    Error,
}

impl FetchOutcome {
    fn as_str(self) -> &'static str {
        match self {
            Self::Content => "content",
            Self::Empty => "empty",
            Self::Error => "error",
        }
    }
}
