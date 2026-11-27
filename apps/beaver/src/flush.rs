use std::time::Duration;

use aws_credential_types::provider::{ProvideCredentials, SharedCredentialsProvider};
use color_eyre::eyre::{Result, WrapErr};
use foundation_recurring_job::{Job, Schedule};
use jiff::Timestamp;

use crate::store::{LogStore, S3Credentials};

const SECONDS_PER_HOUR: i64 = 3600;

/// Hourly, to write a few large files instead of many small ones.
const FLUSH_INTERVAL: Duration = Duration::from_secs(SECONDS_PER_HOUR as u64);

/// Writes logs from completed hours to storage so they don't accumulate in memory.
#[derive(Clone)]
pub struct FlushJob {
    store: LogStore,
    /// Only set when flushing to S3, as a local directory needs no credentials.
    credentials: Option<SharedCredentialsProvider>,
}

impl FlushJob {
    pub fn new(store: LogStore, credentials: Option<SharedCredentialsProvider>) -> Self {
        Self { store, credentials }
    }

    /// Flushes every log, including the current hour, for use as the process shuts down.
    pub async fn flush_everything(&self) -> Result<usize> {
        self.flush(None).await
    }

    async fn flush(&self, before: Option<Timestamp>) -> Result<usize> {
        self.refresh_credentials().await?;

        let flushed = self.store.flush(before).await?;

        if flushed > 0 {
            tracing::info!(flushed, ?before, "flushed logs to storage");
        }

        Ok(flushed)
    }

    /// The credentials are temporary, so they are renewed before each flush.
    async fn refresh_credentials(&self) -> Result<()> {
        let Some(provider) = &self.credentials else {
            return Ok(());
        };

        let credentials = provider
            .provide_credentials()
            .await
            .wrap_err("failed to load AWS credentials")?;

        self.store
            .set_s3_credentials(S3Credentials {
                access_key_id: credentials.access_key_id().to_owned(),
                secret_access_key: credentials.secret_access_key().to_owned(),
                session_token: credentials.session_token().map(str::to_owned),
            })
            .await
    }
}

impl Job for FlushJob {
    const NAME: &'static str = "flush-logs";

    async fn run(&self) -> Result<()> {
        self.flush(Some(start_of_hour(Timestamp::now())?)).await?;

        Ok(())
    }

    fn schedule(&self) -> Schedule {
        // Each run flushes every completed hour, so when in the hour it runs doesn't matter
        Schedule::interval(FLUSH_INTERVAL)
    }
}

/// The start of the hour containing `timestamp`, before which every hour is complete.
fn start_of_hour(timestamp: Timestamp) -> Result<Timestamp> {
    let seconds = timestamp.as_second();

    Ok(Timestamp::from_second(
        seconds - seconds.rem_euclid(SECONDS_PER_HOUR),
    )?)
}

#[cfg(test)]
mod tests {
    use color_eyre::eyre::Result;
    use jiff::Timestamp;

    use crate::flush::start_of_hour;

    #[test]
    fn hours_are_truncated() -> Result<()> {
        let start = |value: &str| -> Result<Timestamp> { start_of_hour(value.parse()?) };

        assert_eq!(
            start("2026-11-24T10:59:59.999Z")?,
            "2026-11-24T10:00:00Z".parse()?
        );
        assert_eq!(
            start("2026-11-24T11:00:00Z")?,
            "2026-11-24T11:00:00Z".parse()?
        );
        assert_eq!(
            start("2026-11-24T00:00:01Z")?,
            "2026-11-24T00:00:00Z".parse()?
        );

        Ok(())
    }
}
