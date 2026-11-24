use std::time::Duration;

use opentelemetry::KeyValue;
use opentelemetry::metrics::{Counter, Histogram, Meter};

/// Instruments describing the batches of logs received from `vector`.
#[derive(Clone)]
pub struct IngestMetrics {
    requests: Counter<u64>,
    logs: Counter<u64>,
    batch_size: Histogram<u64>,
    duration: Histogram<f64>,
}

impl IngestMetrics {
    pub fn new(meter: &Meter) -> Self {
        Self {
            requests: meter
                .u64_counter("beaver_ingest_requests_total")
                .with_description("Total batches received from vector by outcome")
                .build(),
            logs: meter
                .u64_counter("beaver_ingest_logs_total")
                .with_description("Total logs stored")
                .build(),
            batch_size: meter
                .u64_histogram("beaver_ingest_batch_size")
                .with_description("Number of logs in each batch that was stored")
                .build(),
            duration: meter
                .f64_histogram("beaver_ingest_duration_ms")
                .with_description("Time taken to insert a batch in milliseconds")
                .build(),
        }
    }

    /// Record a batch that could not be understood, so was never inserted.
    pub fn invalid(&self) {
        self.requests
            .add(1, &[KeyValue::new("outcome", Outcome::Invalid.as_str())]);
    }

    /// Record a batch that was inserted successfully.
    pub fn stored(&self, count: usize, elapsed: Duration) {
        self.requests
            .add(1, &[KeyValue::new("outcome", Outcome::Stored.as_str())]);
        self.logs.add(count as u64, &[]);
        self.batch_size.record(count as u64, &[]);
        self.duration.record(
            elapsed.as_secs_f64() * 1000.0,
            &[KeyValue::new("outcome", Outcome::Stored.as_str())],
        );
    }

    /// Record a batch which failed to be inserted.
    pub fn failed(&self, elapsed: Duration) {
        self.requests
            .add(1, &[KeyValue::new("outcome", Outcome::Failed.as_str())]);
        self.duration.record(
            elapsed.as_secs_f64() * 1000.0,
            &[KeyValue::new("outcome", Outcome::Failed.as_str())],
        );
    }
}

#[derive(Clone, Copy, Debug)]
enum Outcome {
    /// The batch was inserted.
    Stored,
    /// The body was not a valid batch of logs.
    Invalid,
    /// The batch was valid but inserting it failed.
    Failed,
}

impl Outcome {
    fn as_str(self) -> &'static str {
        match self {
            Self::Stored => "stored",
            Self::Invalid => "invalid",
            Self::Failed => "failed",
        }
    }
}
