use std::future::Future;
use std::time::Duration;

use color_eyre::eyre::Result;
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

impl<T: Job> GracefulTask for RecurringJob<T> {
    async fn run_until_shutdown(self, shutdown: CancellationToken) -> Result<()> {
        let Schedule::Interval(duration) = self.state.schedule() else {
            unimplemented!("only interval scheduling is supported for now");
        };

        let mut interval = tokio::time::interval(duration);
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
}

#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::Duration;

    use color_eyre::eyre::Result;
    use foundation_shutdown::{CancellationToken, GracefulTask};

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
    async fn can_create_and_run_recurring_job() {
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

        tokio::spawn(async move {
            job.run_until_shutdown(job_token).await.unwrap();
        });

        // advance time to allow the job to run a few times
        let iterations = 3;
        tokio::time::sleep(interval * iterations).await;

        assert_eq!(counter.load(Ordering::SeqCst), iterations);

        shutdown_token.cancel();
    }
}
