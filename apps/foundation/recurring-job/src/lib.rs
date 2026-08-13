use std::future::Future;
use std::time::Duration;

use chrono::{DateTime, Datelike, NaiveTime, TimeZone, Timelike, Utc, Weekday};
use color_eyre::eyre::{eyre, Result};
use foundation_shutdown::{CancellationToken, GracefulTask};
use tokio::time::{Instant, Interval};

#[derive(Copy, Clone, Debug)]
pub enum Schedule {
    Interval(Duration),
    Daily { time: NaiveTime },
    Weekly { day: Weekday, time: NaiveTime },
}

impl Schedule {
    pub fn interval(duration: Duration) -> Self {
        Schedule::Interval(duration)
    }

    pub fn daily(time: NaiveTime) -> Self {
        Schedule::Daily { time }
    }

    pub fn weekly(day: Weekday, time: NaiveTime) -> Self {
        Schedule::Weekly { day, time }
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
    async fn run_on_interval(
        self,
        mut interval: Interval,
        shutdown: CancellationToken,
    ) -> Result<()> {
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

    async fn run_interval_job(self, interval: Duration, shutdown: CancellationToken) -> Result<()> {
        let interval = tokio::time::interval(interval);

        self.run_on_interval(interval, shutdown).await
    }

    async fn run_daily_job(
        self,
        target_time: NaiveTime,
        shutdown: CancellationToken,
    ) -> Result<()> {
        let now = Utc::now();
        let initial_delay = calculate_initial_delay(now, target_time)?;

        let start = Instant::now() + initial_delay;
        let interval = tokio::time::interval_at(start, Duration::from_hours(24));

        self.run_on_interval(interval, shutdown).await
    }

    async fn run_weekly_job(
        self,
        target_day: Weekday,
        target_time: NaiveTime,
        shutdown: CancellationToken,
    ) -> Result<()> {
        let now = Utc::now();
        let current_time = now.time();
        let current_date = now.date_naive();
        let today = current_date.weekday();

        let target_date = if target_day == today && current_time < target_time {
            current_date
        } else {
            let (mut weekday, mut date) = (today, current_date);

            while weekday != target_day {
                weekday = weekday.succ();
                date = date
                    .succ_opt()
                    .ok_or_else(|| eyre!("failed to get the next date for a recurring job"))?;
            }

            date
        };

        let next_run = target_date.and_time(target_time).and_utc();
        let duration_until_next_run = next_run - now;
        let sleep_duration = duration_until_next_run.to_std().unwrap_or_default();

        let start = Instant::now() + sleep_duration;
        let interval = tokio::time::interval_at(start, Duration::from_hours(24 * 7));

        self.run_on_interval(interval, shutdown).await
    }
}

impl<T: Job> GracefulTask for RecurringJob<T> {
    async fn run_until_shutdown(self, shutdown: CancellationToken) -> Result<()> {
        match self.state.schedule() {
            Schedule::Interval(duration) => {
                tracing::info!(job = T::NAME, ?duration, "starting recurring job");
                self.run_interval_job(duration, shutdown).await
            }
            Schedule::Daily { time } => {
                tracing::info!(job = T::NAME, ?time, "starting recurring job");
                self.run_daily_job(time, shutdown).await
            }
            Schedule::Weekly { day, time } => {
                tracing::info!(job = T::NAME, ?day, ?time, "starting recurring job");
                self.run_weekly_job(day, time, shutdown).await
            }
        }
    }
}

fn calculate_next_run(now: DateTime<Utc>, target_time: NaiveTime) -> Result<DateTime<Utc>> {
    let target_hour = target_time.hour();
    let target_minute = target_time.minute();

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

fn calculate_initial_delay(now: DateTime<Utc>, target_time: NaiveTime) -> Result<Duration> {
    let next_run = calculate_next_run(now, target_time)?;
    let duration_until_next_run = next_run - now;
    let duration_until_next_run = duration_until_next_run.to_std().unwrap_or_default();

    Ok(duration_until_next_run)
}

#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    use chrono::{DateTime, Datelike, NaiveTime, Utc};
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

        let time = NaiveTime::from_hms_opt(9, 0, 0).unwrap();
        let schedule = Schedule::daily(time);

        let job = TestJob {
            counter: counter.clone(),
            schedule,
        };

        let job = RecurringJob::new(job);

        let shutdown_token = CancellationToken::new();
        let job_token = shutdown_token.clone();

        let handle = tokio::spawn(async move { job.run_until_shutdown(job_token).await });

        // advance time to allow the job to run
        let target_time = NaiveTime::from_hms_opt(9, 0, 0).unwrap();
        let sleep_duration = crate::calculate_initial_delay(Utc::now(), target_time)?;
        tokio::time::sleep(sleep_duration + Duration::from_millis(1)).await;

        assert_eq!(counter.load(Ordering::SeqCst), 1);

        shutdown_token.cancel();

        handle.await??;

        Ok(())
    }

    #[tokio::test(start_paused = true)]
    async fn can_handle_weekly_jobs() -> Result<()> {
        let counter = Arc::new(AtomicU32::new(0));

        let current_day = Utc::now().date_naive().weekday();
        let two_days_time = current_day.succ().succ();

        let time = NaiveTime::from_hms_opt(9, 0, 0).unwrap();
        let schedule = Schedule::weekly(two_days_time, time);

        let job = TestJob {
            counter: counter.clone(),
            schedule,
        };

        let job = RecurringJob::new(job);

        let shutdown_token = CancellationToken::new();
        let job_token = shutdown_token.clone();

        let handle = tokio::spawn(async move { job.run_until_shutdown(job_token).await });

        // move time forward a little bit
        tokio::time::sleep(Duration::from_mins(1)).await;

        // check it still hasn't run
        assert_eq!(counter.load(Ordering::SeqCst), 0);

        // move forward until the job will have run
        tokio::time::sleep(Duration::from_hours(24 * 3)).await;

        // check it has now run
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

    fn time(time: &str) -> NaiveTime {
        let parts: Vec<&str> = time.split(':').collect();

        let hour = parts[0].parse::<u32>().expect("Failed to parse hour");
        let minute = parts[1].parse::<u32>().expect("Failed to parse minute");

        NaiveTime::from_hms_opt(hour, minute, 0).unwrap()
    }

    #[test_case(datetime("01/06/2026", "08:00"), datetime("01/06/2026", "09:00"), time("09:00"); "before scheduled time")]
    #[test_case(datetime("01/06/2026", "08:59"), datetime("01/06/2026", "09:00"), time("09:00"); "just before scheduled time")]
    #[test_case(datetime("01/06/2026", "09:01"), datetime("02/06/2026", "09:00"), time("09:00"); "just after scheduled time")]
    #[test_case(datetime("01/06/2026", "10:00"), datetime("02/06/2026", "09:00"), time("09:00"); "after scheduled time")]
    fn can_calculate_next_run(
        now: DateTime<Utc>,
        expected: DateTime<Utc>,
        scheduled_time: NaiveTime,
    ) {
        let next_run = crate::calculate_next_run(now, scheduled_time)
            .expect("Failed to calculate next run time");

        assert_eq!(next_run, expected);
    }
}
