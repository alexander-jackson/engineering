//! Autonomous domain classification.
//!
//! The first time a domain is seen, its root document is fetched ([`DomainFetcher`]), reduced to
//! plain text and scored by a decision model ([`DomainDecider`]) against the configured
//! questions. The resulting verdict is persisted so it is only computed once.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use color_eyre::eyre::Result;
use tokio::sync::watch;

use crate::classifier::metrics::{FetchOutcome, JobOutcome, LookupResult};
use crate::uid::QuestionUid;

mod decider;
mod fetcher;
mod metrics;
mod question;
mod rollout;
mod store;

pub use crate::classifier::decider::OpenRouterDecider;
pub use crate::classifier::fetcher::ReqwestFetcher;
pub use crate::classifier::metrics::ClassifierMetrics;
pub use crate::classifier::question::{Prompt, Question, Threshold};
pub use crate::classifier::rollout::Rollout;
pub use crate::classifier::store::PostgresDecisionStore;

/// The maximum time a background classification may run for, regardless of how long callers wait.
const JOB_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Verdict {
    Allowed,
    Blocked,
}

impl Verdict {
    fn as_str(self) -> &'static str {
        match self {
            Self::Allowed => "allowed",
            Self::Blocked => "blocked",
        }
    }
}

#[async_trait::async_trait]
pub trait DomainFetcher: Send + Sync {
    /// Fetch the text content of the root document for `domain`, or `None` if there isn't one.
    async fn fetch(&self, domain: &str) -> Result<Option<String>>;
}

#[async_trait::async_trait]
pub trait DomainDecider: Send + Sync {
    /// Score `content` against each of the `questions`, returning one score in `0.0..=1.0` per
    /// question, in order.
    async fn decide(&self, domain: &str, content: &str, questions: &[Question])
    -> Result<Vec<f64>>;
}

#[async_trait::async_trait]
pub trait DecisionStore: Send + Sync {
    async fn questions(&self) -> Result<Vec<Question>>;
    async fn decisions(&self) -> Result<HashMap<String, bool>>;
    /// Record the decision for a domain, along with the score each question gave it for auditing.
    async fn record(
        &self,
        domain: &str,
        blocked: bool,
        scores: &[(QuestionUid, f64)],
    ) -> Result<()>;

    /// Add a new question. Questions are immutable once created.
    async fn add_question(&self, prompt: &Prompt, threshold: Threshold) -> Result<QuestionUid>;

    /// Retire a question so it is no longer asked, returning whether an active one was found.
    async fn retire_question(&self, uid: QuestionUid) -> Result<bool>;

    /// Atomically mark a question as redundant and add a new one in its place, returning the new question's
    /// UID, or `None` if there was no active question to replace.
    async fn replace_question(
        &self,
        uid: QuestionUid,
        prompt: &Prompt,
        threshold: Threshold,
    ) -> Result<Option<QuestionUid>>;
}

/// Object-safe view of a classifier, so request handlers don't need to know its components.
#[async_trait::async_trait]
pub trait Classify: Send + Sync {
    async fn classify(&self, domain: &str) -> Verdict;
}

type InFlight = Arc<Mutex<HashMap<String, watch::Receiver<Option<Verdict>>>>>;

pub struct DomainClassifier<F, D, S> {
    fetcher: Arc<F>,
    decider: Arc<D>,
    store: Arc<S>,
    decisions: Arc<RwLock<HashMap<String, bool>>>,
    in_flight: InFlight,
    wait_timeout: Duration,
    metrics: ClassifierMetrics,
    rollout: Rollout,
}

impl<F, D, S> DomainClassifier<F, D, S>
where
    F: DomainFetcher + 'static,
    D: DomainDecider + 'static,
    S: DecisionStore + 'static,
{
    /// Create a classifier, loading any previously stored decisions from `store`.
    ///
    /// Only new domains that are part of `rollout` are classified, and the rest are allowed
    /// without being looked at. Domains with a stored decision are enforced regardless.
    pub async fn new(
        fetcher: F,
        decider: D,
        store: S,
        wait_timeout: Duration,
        metrics: ClassifierMetrics,
        rollout: Rollout,
    ) -> Result<Self> {
        let decisions = store.decisions().await?;

        Ok(Self {
            fetcher: Arc::new(fetcher),
            decider: Arc::new(decider),
            store: Arc::new(store),
            decisions: Arc::new(RwLock::new(decisions)),
            in_flight: Arc::default(),
            wait_timeout,
            metrics,
            rollout,
        })
    }

    fn known(&self, domain: &str) -> Option<Verdict> {
        let decisions = self.decisions.read().unwrap();

        decisions.get(domain).map(|blocked| match blocked {
            true => Verdict::Blocked,
            false => Verdict::Allowed,
        })
    }

    /// Get a receiver for the classification of `domain`, starting a background job for it if
    /// one isn't already running.
    fn subscribe(&self, domain: &str) -> watch::Receiver<Option<Verdict>> {
        let mut in_flight = self.in_flight.lock().unwrap();

        if let Some(rx) = in_flight.get(domain) {
            return rx.clone();
        }

        let (tx, rx) = watch::channel(None);
        in_flight.insert(domain.to_string(), rx.clone());

        let job = Job {
            fetcher: Arc::clone(&self.fetcher),
            decider: Arc::clone(&self.decider),
            store: Arc::clone(&self.store),
            decisions: Arc::clone(&self.decisions),
            domain: domain.to_string(),
            metrics: self.metrics.clone(),
        };
        let in_flight = Arc::clone(&self.in_flight);

        tokio::spawn(async move {
            let start = Instant::now();

            let (outcome, verdict) = match tokio::time::timeout(JOB_TIMEOUT, job.run()).await {
                Ok(result) => result,
                Err(_) => {
                    tracing::warn!(domain = %job.domain, "classification timed out");
                    (JobOutcome::TimedOut, Verdict::Allowed)
                }
            };

            job.metrics.job(outcome, verdict, elapsed_ms(start));

            // Remove the entry before publishing so a later query sees the stored decision (or
            // starts a fresh job) rather than a finished channel.
            in_flight.lock().unwrap().remove(&job.domain);
            let _ = tx.send(Some(verdict));
        });

        rx
    }
}

#[async_trait::async_trait]
impl<F, D, S> Classify for DomainClassifier<F, D, S>
where
    F: DomainFetcher + 'static,
    D: DomainDecider + 'static,
    S: DecisionStore + 'static,
{
    #[tracing::instrument(skip(self))]
    async fn classify(&self, domain: &str) -> Verdict {
        let domain = domain.trim_end_matches('.').to_lowercase();

        if !is_classifiable(&domain) {
            tracing::debug!("decided that the input was unclassifiable");
            self.metrics
                .lookup(LookupResult::Unclassifiable, Verdict::Allowed);
            return Verdict::Allowed;
        }

        if let Some(verdict) = self.known(&domain) {
            tracing::debug!(?verdict, "input was already known");
            self.metrics.lookup(LookupResult::Known, verdict);
            return verdict;
        }

        if !self.rollout.includes(&domain) {
            tracing::debug!("domain is not part of the rollout");
            self.metrics
                .lookup(LookupResult::NotInRollout, Verdict::Allowed);
            return Verdict::Allowed;
        }

        let mut rx = self.subscribe(&domain);
        let wait = rx.wait_for(|verdict| verdict.is_some());

        let (result, verdict) = match tokio::time::timeout(self.wait_timeout, wait).await {
            Ok(Ok(verdict)) => (LookupResult::Completed, verdict.unwrap_or(Verdict::Allowed)),
            Ok(Err(_)) => (LookupResult::Completed, Verdict::Allowed),
            Err(_) => {
                tracing::warn!(%domain, "timed out waiting for classification, allowing");
                (LookupResult::WaitTimeout, Verdict::Allowed)
            }
        };

        self.metrics.lookup(result, verdict);
        verdict
    }
}

struct Job<F, D, S> {
    fetcher: Arc<F>,
    decider: Arc<D>,
    store: Arc<S>,
    decisions: Arc<RwLock<HashMap<String, bool>>>,
    domain: String,
    metrics: ClassifierMetrics,
}

impl<F: DomainFetcher, D: DomainDecider, S: DecisionStore> Job<F, D, S> {
    async fn run(&self) -> (JobOutcome, Verdict) {
        let questions = match self.store.questions().await {
            Ok(questions) if questions.is_empty() => {
                return (JobOutcome::NoQuestions, Verdict::Allowed);
            }
            Ok(questions) => questions,
            Err(e) => {
                tracing::error!(error = ?e, domain = %self.domain, "failed to load questions");
                return (JobOutcome::QuestionsFailed, Verdict::Allowed);
            }
        };

        let start = Instant::now();
        let fetched = self.fetcher.fetch(&self.domain).await;

        let content = match fetched {
            Ok(Some(content)) if !content.is_empty() => {
                self.metrics.fetch(FetchOutcome::Content, elapsed_ms(start));
                content
            }
            Ok(_) => {
                tracing::debug!(domain = %self.domain, "no content to classify, allowing");
                self.metrics.fetch(FetchOutcome::Empty, elapsed_ms(start));
                return (JobOutcome::NoContent, self.finish(false, &[]).await);
            }
            Err(e) => {
                tracing::debug!(error = ?e, domain = %self.domain, "failed to fetch, allowing");
                self.metrics.fetch(FetchOutcome::Error, elapsed_ms(start));
                return (JobOutcome::FetchFailed, self.finish(false, &[]).await);
            }
        };

        let start = Instant::now();
        let decided = self
            .decider
            .decide(&self.domain, &content, &questions)
            .await;
        self.metrics.decision(decided.is_ok(), elapsed_ms(start));

        let scores = match decided {
            Ok(scores) => scores,
            Err(e) => {
                // Don't record anything, so the domain is evaluated again next time.
                tracing::error!(error = ?e, domain = %self.domain, "decision model failed");
                return (JobOutcome::DecisionFailed, Verdict::Allowed);
            }
        };

        let blocked = questions
            .iter()
            .zip(&scores)
            .any(|(question, score)| *score > *question.threshold);

        let scores: Vec<_> = questions
            .iter()
            .zip(&scores)
            .map(|(question, score)| (question.uid, *score))
            .collect();

        tracing::info!(domain = %self.domain, blocked, ?scores, "classified domain");

        (JobOutcome::Scored, self.finish(blocked, &scores).await)
    }

    async fn finish(&self, blocked: bool, scores: &[(QuestionUid, f64)]) -> Verdict {
        if let Err(e) = self.store.record(&self.domain, blocked, scores).await {
            tracing::error!(error = ?e, domain = %self.domain, "failed to record decision");
        }

        self.decisions
            .write()
            .unwrap()
            .insert(self.domain.clone(), blocked);

        if blocked {
            Verdict::Blocked
        } else {
            Verdict::Allowed
        }
    }
}

fn elapsed_ms(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}

/// Whether it makes sense to try fetching a website for `domain`.
fn is_classifiable(domain: &str) -> bool {
    const INTERNAL_SUFFIXES: [&str; 5] = [".arpa", ".local", ".localhost", ".internal", ".lan"];

    domain.contains('.')
        && !INTERNAL_SUFFIXES
            .iter()
            .any(|suffix| domain.ends_with(suffix))
        && domain.parse::<std::net::IpAddr>().is_err()
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    struct StubFetcher {
        content: Option<String>,
        calls: Arc<AtomicUsize>,
        delay: Duration,
    }

    #[async_trait::async_trait]
    impl DomainFetcher for StubFetcher {
        async fn fetch(&self, _: &str) -> Result<Option<String>> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            tokio::time::sleep(self.delay).await;
            Ok(self.content.clone())
        }
    }

    struct StubDecider(Result<Vec<f64>, &'static str>);

    #[async_trait::async_trait]
    impl DomainDecider for StubDecider {
        async fn decide(&self, _: &str, _: &str, _: &[Question]) -> Result<Vec<f64>> {
            self.0.clone().map_err(|e| color_eyre::eyre::eyre!(e))
        }
    }

    #[derive(Default)]
    struct MemoryStore {
        questions: Mutex<Vec<Question>>,
        recorded: Mutex<HashMap<String, bool>>,
        scores: Mutex<HashMap<String, Vec<(QuestionUid, f64)>>>,
    }

    #[async_trait::async_trait]
    impl DecisionStore for Arc<MemoryStore> {
        async fn questions(&self) -> Result<Vec<Question>> {
            Ok(self.questions.lock().unwrap().clone())
        }

        async fn decisions(&self) -> Result<HashMap<String, bool>> {
            Ok(self.recorded.lock().unwrap().clone())
        }

        async fn record(
            &self,
            domain: &str,
            blocked: bool,
            scores: &[(QuestionUid, f64)],
        ) -> Result<()> {
            self.recorded
                .lock()
                .unwrap()
                .insert(domain.to_string(), blocked);
            self.scores
                .lock()
                .unwrap()
                .insert(domain.to_string(), scores.to_vec());
            Ok(())
        }

        async fn add_question(&self, prompt: &Prompt, threshold: Threshold) -> Result<QuestionUid> {
            let uid = QuestionUid::new();
            self.questions.lock().unwrap().push(Question {
                uid,
                prompt: prompt.clone(),
                threshold,
            });

            Ok(uid)
        }

        async fn retire_question(&self, uid: QuestionUid) -> Result<bool> {
            let mut questions = self.questions.lock().unwrap();
            let before = questions.len();
            questions.retain(|q| q.uid != uid);

            Ok(questions.len() != before)
        }

        async fn replace_question(
            &self,
            uid: QuestionUid,
            prompt: &Prompt,
            threshold: Threshold,
        ) -> Result<Option<QuestionUid>> {
            if !self.retire_question(uid).await? {
                return Ok(None);
            }

            self.add_question(prompt, threshold).await.map(Some)
        }
    }

    fn question(threshold: f64) -> Question {
        Question {
            uid: QuestionUid::new(),
            prompt: Prompt::try_from("is it bad?").unwrap(),
            threshold: Threshold::try_from(threshold).unwrap(),
        }
    }

    struct Harness {
        classifier: DomainClassifier<StubFetcher, StubDecider, Arc<MemoryStore>>,
        store: Arc<MemoryStore>,
        fetches: Arc<AtomicUsize>,
    }

    async fn harness(
        content: Option<&str>,
        scores: Result<Vec<f64>, &'static str>,
        questions: Vec<Question>,
        delay: Duration,
        wait: Duration,
        rollout: Rollout,
    ) -> Harness {
        let fetches = Arc::new(AtomicUsize::new(0));
        let store = Arc::new(MemoryStore {
            questions: Mutex::new(questions),
            ..Default::default()
        });
        let fetcher = StubFetcher {
            content: content.map(str::to_string),
            calls: Arc::clone(&fetches),
            delay,
        };
        let classifier = DomainClassifier::new(
            fetcher,
            StubDecider(scores),
            Arc::clone(&store),
            wait,
            ClassifierMetrics::new(&opentelemetry::global::meter("test")),
            rollout,
        )
        .await
        .unwrap();

        Harness {
            classifier,
            store,
            fetches,
        }
    }

    async fn simple(content: Option<&str>, scores: Result<Vec<f64>, &'static str>) -> Harness {
        harness(
            content,
            scores,
            vec![question(0.5)],
            Duration::ZERO,
            Duration::from_secs(5),
            Rollout::ALL,
        )
        .await
    }

    #[tokio::test]
    async fn blocks_when_a_score_exceeds_its_threshold() {
        let h = simple(Some("bad stuff"), Ok(vec![0.9])).await;

        assert_eq!(
            h.classifier.classify("bad.example.").await,
            Verdict::Blocked
        );
        assert!(h.store.recorded.lock().unwrap()["bad.example"]);
    }

    #[tokio::test]
    async fn decisions_record_the_score_of_each_question() {
        let questions = vec![question(0.5), question(0.5)];
        let uids: Vec<_> = questions.iter().map(|q| q.uid).collect();
        let h = harness(
            Some("content"),
            Ok(vec![0.2, 0.9]),
            questions,
            Duration::ZERO,
            Duration::from_secs(5),
            Rollout::ALL,
        )
        .await;

        h.classifier.classify("bad.example").await;

        let scores = h.store.scores.lock().unwrap()["bad.example"].clone();
        assert_eq!(scores, vec![(uids[0], 0.2), (uids[1], 0.9)]);
    }

    #[tokio::test]
    async fn allows_when_scores_are_under_thresholds() {
        let h = simple(Some("fine"), Ok(vec![0.1])).await;

        assert_eq!(h.classifier.classify("ok.example").await, Verdict::Allowed);
        assert!(!h.store.recorded.lock().unwrap()["ok.example"]);
    }

    #[tokio::test]
    async fn a_score_equal_to_the_threshold_is_allowed() {
        let h = simple(Some("meh"), Ok(vec![0.5])).await;

        assert_eq!(h.classifier.classify("ok.example").await, Verdict::Allowed);
    }

    #[tokio::test]
    async fn decisions_are_only_made_once() {
        let h = simple(Some("bad stuff"), Ok(vec![0.9])).await;

        h.classifier.classify("bad.example").await;
        let second = h.classifier.classify("BAD.example").await;

        assert_eq!(second, Verdict::Blocked);
        assert_eq!(h.fetches.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn missing_content_is_allowed_and_recorded() {
        let h = simple(None, Ok(vec![0.9])).await;

        assert_eq!(h.classifier.classify("api.example").await, Verdict::Allowed);
        assert!(!h.store.recorded.lock().unwrap()["api.example"]);
        assert!(h.store.scores.lock().unwrap()["api.example"].is_empty());
    }

    #[tokio::test]
    async fn model_failure_is_allowed_but_not_recorded() {
        let h = simple(Some("content"), Err("openrouter down")).await;

        assert_eq!(h.classifier.classify("a.example").await, Verdict::Allowed);
        assert!(h.store.recorded.lock().unwrap().is_empty());

        h.classifier.classify("a.example").await;
        assert_eq!(h.fetches.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn no_questions_means_nothing_is_fetched() {
        let h = harness(
            Some("x"),
            Ok(vec![]),
            vec![],
            Duration::ZERO,
            Duration::from_secs(5),
            Rollout::ALL,
        )
        .await;

        assert_eq!(h.classifier.classify("a.example").await, Verdict::Allowed);
        assert_eq!(h.fetches.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn internal_names_are_not_classified() {
        let h = simple(Some("bad"), Ok(vec![1.0])).await;

        for name in [
            "localhost",
            "printer.local",
            "1.0.0.127.in-addr.arpa",
            "10.0.0.1",
        ] {
            assert_eq!(h.classifier.classify(name).await, Verdict::Allowed);
        }
        assert_eq!(h.fetches.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn timeout_allows_but_the_job_still_records_its_verdict() {
        let h = harness(
            Some("bad stuff"),
            Ok(vec![0.9]),
            vec![question(0.5)],
            Duration::from_millis(200),
            Duration::from_millis(20),
            Rollout::ALL,
        )
        .await;

        assert_eq!(
            h.classifier.classify("slow.example").await,
            Verdict::Allowed
        );

        tokio::time::sleep(Duration::from_millis(400)).await;
        assert_eq!(
            h.classifier.classify("slow.example").await,
            Verdict::Blocked
        );
        assert_eq!(h.fetches.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn concurrent_queries_share_one_job() {
        let h = harness(
            Some("bad stuff"),
            Ok(vec![0.9]),
            vec![question(0.5)],
            Duration::from_millis(100),
            Duration::from_secs(5),
            Rollout::ALL,
        )
        .await;

        let (a, b, c) = tokio::join!(
            h.classifier.classify("shared.example"),
            h.classifier.classify("shared.example"),
            h.classifier.classify("shared.example"),
        );

        assert_eq!([a, b, c], [Verdict::Blocked; 3]);
        assert_eq!(h.fetches.load(Ordering::SeqCst), 1);
    }

    async fn outside_rollout() -> Harness {
        harness(
            Some("bad stuff"),
            Ok(vec![0.9]),
            vec![question(0.5)],
            Duration::ZERO,
            Duration::from_secs(5),
            Rollout::try_from(0).unwrap(),
        )
        .await
    }

    #[tokio::test]
    async fn domains_outside_the_rollout_are_allowed_without_being_fetched() {
        let h = outside_rollout().await;

        assert_eq!(h.classifier.classify("bad.example").await, Verdict::Allowed);
        assert_eq!(h.fetches.load(Ordering::SeqCst), 0);
        assert!(h.store.recorded.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn known_decisions_are_enforced_outside_the_rollout() {
        let h = outside_rollout().await;
        h.classifier
            .decisions
            .write()
            .unwrap()
            .insert("bad.example".to_string(), true);

        assert_eq!(h.classifier.classify("bad.example").await, Verdict::Blocked);
    }
}
