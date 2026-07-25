use std::future::Future;
use std::time::Duration;

use chrono::{DateTime, TimeZone, Utc};
use color_eyre::eyre::{Result, eyre};
use foundation_shutdown::{CancellationToken, GracefulTask};

#[derive(Copy, Clone, Debug)]
pub enum Schedule {
    Interval(Duration),
    Daily { hour: u32, minute: u32 },
}

impl Schedule {
    pub fn interval(duration: Duration) -> Self {
        Schedule::Interval(duration)
    }

    pub fn daily(hour: u32, minute: u32) -> Self {
        Schedule::Daily { hour, minute }
    }
}

pub trait Job: Send + 'static {
    const NAME: &'static str;

    fn run(&self) -> impl Future<Output = Result<()>> + Send + '_;
    fn schedule(&self) -> Schedule;
}

pub struct RecurringJob<T>
where
    T: Job,
{
    state: T,
}

impl<T: Job> RecurringJob<T> {
    pub fn new(state: T) -> Self {
        Self { state }
    }
}

impl<T: Job> RecurringJob<T> {
    async fn run_interval_job(self, interval: Duration, shutdown: CancellationToken) -> Result<()> {
        let mut interval = tokio::time::interval(interval);
        let job = T::NAME;

        loop {
            tokio::select! {
                _ = shutdown.cancelled() => {
                    tracing::info!(%job, "shutting down gracefully");
                    break;
                }
                _ = interval.tick() => {
                    if let Err(e) = self.state.run().await {
                        tracing::warn!(%job, error = ?e, "job execution failed");
                    }
                }
            }
        }

        Ok(())
    }

    async fn run_daily_job(
        self,
        hour: u32,
        minute: u32,
        shutdown: CancellationToken,
    ) -> Result<()> {
        let job = T::NAME;

        loop {
            let now = Utc::now();
            let sleep_duration = calculate_sleep_duration(now, hour, minute)?;

            tracing::info!(%job, ?sleep_duration, "waiting until next scheduled run");

            tokio::select! {
                _ = shutdown.cancelled() => {
                    tracing::info!(%job, "shutting down gracefully");
                    break;
                }
                _ = tokio::time::sleep(sleep_duration) => {
                    if let Err(e) = self.state.run().await {
                        tracing::warn!(%job, error = ?e, "job execution failed");
                    }
                }
            }
        }

        Ok(())
    }
}

impl<T: Job> GracefulTask for RecurringJob<T> {
    async fn run_until_shutdown(self, shutdown: CancellationToken) -> Result<()> {
        match self.state.schedule() {
            Schedule::Interval(duration) => {
                tracing::info!(job = T::NAME, ?duration, "starting recurring job");
                self.run_interval_job(duration, shutdown).await
            }
            Schedule::Daily { hour, minute } => {
                tracing::info!(job = T::NAME, ?hour, ?minute, "starting recurring job");
                self.run_daily_job(hour, minute, shutdown).await
            }
        }
    }
}

fn calculate_next_run(
    now: DateTime<Utc>,
    target_hour: u32,
    target_minute: u32,
) -> Result<DateTime<Utc>> {
    let current_date = now.date_naive();
    let target_time = current_date
        .and_hms_opt(target_hour, target_minute, 0)
        .ok_or_else(|| eyre!("Invalid target hour or minute"))?;

    // If the target time has already passed today, schedule for tomorrow
    let next_run = if now.naive_utc() >= target_time {
        current_date
            .succ_opt()
            .ok_or_else(|| eyre!("Failed to calculate next day"))?
            .and_hms_opt(target_hour, target_minute, 0)
            .ok_or_else(|| eyre!("Invalid target hour or minute for next day"))?
    } else {
        target_time
    };

    let next_run_utc = Utc
        .from_local_datetime(&next_run)
        .single()
        .ok_or_else(|| eyre!("Failed to convert next run time to UTC"))?;

    Ok(next_run_utc)
}

fn calculate_sleep_duration(
    now: DateTime<Utc>,
    target_hour: u32,
    target_minute: u32,
) -> Result<Duration> {
    let next_run = calculate_next_run(now, target_hour, target_minute)?;
    let duration_until_next_run = next_run - now;
    let duration_until_next_run = duration_until_next_run.to_std().unwrap_or_default();

    Ok(duration_until_next_run)
}

#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::Duration;

    use chrono::{DateTime, Utc};
    use color_eyre::eyre::Result;
    use foundation_shutdown::{CancellationToken, GracefulTask};
    use test_case::test_case;

    use crate::{Job, RecurringJob, Schedule};

    struct TestJob {
        counter: Arc<AtomicU32>,
        schedule: Schedule,
    }

    impl Job for TestJob {
        const NAME: &'static str = "test-job";

        fn schedule(&self) -> Schedule {
            self.schedule
        }

        fn run(&self) -> impl Future<Output = Result<()>> + Send + '_ {
            async move {
                self.counter.fetch_add(1, Ordering::SeqCst);

                Ok(())
            }
        }
    }

    #[tokio::test(start_paused = true)]
    async fn can_handle_scheduled_jobs() -> Result<()> {
        let counter = Arc::new(AtomicU32::new(0));

        let interval = Duration::from_millis(1);
        let schedule = Schedule::interval(interval);

        let job = TestJob {
            counter: counter.clone(),
            schedule,
        };

        let job = RecurringJob::new(job);

        let shutdown_token = CancellationToken::new();
        let job_token = shutdown_token.clone();

        let handle = tokio::spawn(async move { job.run_until_shutdown(job_token).await });

        // advance time to allow the job to run a few times
        let iterations = 3;
        tokio::time::sleep(interval * iterations).await;

        assert_eq!(counter.load(Ordering::SeqCst), iterations);

        shutdown_token.cancel();

        handle.await??;

        Ok(())
    }

    #[tokio::test(start_paused = true)]
    async fn can_handle_daily_jobs() -> Result<()> {
        let counter = Arc::new(AtomicU32::new(0));

        let schedule = Schedule::daily(9, 0);

        let job = TestJob {
            counter: counter.clone(),
            schedule,
        };

        let job = RecurringJob::new(job);

        let shutdown_token = CancellationToken::new();
        let job_token = shutdown_token.clone();

        let handle = tokio::spawn(async move { job.run_until_shutdown(job_token).await });

        // advance time to allow the job to run
        let sleep_duration = crate::calculate_sleep_duration(Utc::now(), 9, 0)?;
        tokio::time::sleep(sleep_duration + Duration::from_millis(1)).await;

        assert_eq!(counter.load(Ordering::SeqCst), 1);

        shutdown_token.cancel();

        handle.await??;

        Ok(())
    }

    fn datetime(date: &str, time: &str) -> DateTime<Utc> {
        let datetime_str = format!("{} {} +0000", date, time);

        DateTime::parse_from_str(&datetime_str, "%d/%m/%Y %H:%M %z")
            .expect("Failed to parse date and time")
            .with_timezone(&Utc)
    }

    fn time(time: &str) -> (u32, u32) {
        let parts: Vec<&str> = time.split(':').collect();

        let hour = parts[0].parse::<u32>().expect("Failed to parse hour");
        let minute = parts[1].parse::<u32>().expect("Failed to parse minute");

        (hour, minute)
    }

    #[test_case(datetime("01/06/2026", "08:00"), datetime("01/06/2026", "09:00"), time("09:00"); "before scheduled time")]
    #[test_case(datetime("01/06/2026", "08:59"), datetime("01/06/2026", "09:00"), time("09:00"); "just before scheduled time")]
    #[test_case(datetime("01/06/2026", "09:01"), datetime("02/06/2026", "09:00"), time("09:00"); "just after scheduled time")]
    #[test_case(datetime("01/06/2026", "10:00"), datetime("02/06/2026", "09:00"), time("09:00"); "after scheduled time")]
    fn can_calculate_next_run(
        now: DateTime<Utc>,
        expected: DateTime<Utc>,
        scheduled_time: (u32, u32),
    ) {
        let (scheduled_hour, scheduled_minute) = scheduled_time;

        let next_run = crate::calculate_next_run(now, scheduled_hour, scheduled_minute)
            .expect("Failed to calculate next run time");

        assert_eq!(next_run, expected);
    }
}
