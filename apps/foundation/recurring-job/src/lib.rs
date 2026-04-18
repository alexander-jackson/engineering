use std::future::Future;
use std::time::Duration;

use color_eyre::eyre::Result;
use foundation_shutdown::{CancellationToken, GracefulTask};

pub trait Job: Send + 'static {
    const NAME: &'static str;
    const INTERVAL: Duration;

    fn run(&self) -> impl Future<Output = Result<()>> + Send + '_;
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
        let mut interval = tokio::time::interval(T::INTERVAL);
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
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    use color_eyre::eyre::Result;
    use foundation_shutdown::{CancellationToken, GracefulTask};
    use tokio::sync::Mutex;
    use tokio::sync::mpsc::{Receiver, Sender};

    use crate::{Job, RecurringJob};

    struct TestJob {
        counter: Arc<AtomicUsize>,
        sender: Sender<()>,
        receiver: Mutex<Receiver<()>>,
    }

    impl Job for TestJob {
        const NAME: &'static str = "test-job";
        const INTERVAL: Duration = Duration::from_millis(1);

        fn run(&self) -> impl Future<Output = Result<()>> + Send + '_ {
            async move {
                self.receiver.lock().await.recv().await;
                self.counter.fetch_add(1, Ordering::SeqCst);
                self.sender.send(()).await.unwrap();

                Ok(())
            }
        }
    }

    #[tokio::test]
    async fn can_create_and_run_recurring_job() {
        let counter = Arc::new(AtomicUsize::new(0));
        let (test_sender, job_receiver) = tokio::sync::mpsc::channel(1);
        let (job_sender, mut test_receiver) = tokio::sync::mpsc::channel(1);

        let job = TestJob {
            counter: counter.clone(),
            sender: job_sender,
            receiver: Mutex::new(job_receiver),
        };

        let job = RecurringJob::new(job);

        let shutdown_token = CancellationToken::new();
        let job_token = shutdown_token.clone();

        tokio::spawn(async move {
            job.run_until_shutdown(job_token).await.unwrap();
        });

        let iterations = 3;

        for _ in 0..iterations {
            test_sender.send(()).await.unwrap();
            test_receiver.recv().await.unwrap();
        }

        assert_eq!(counter.load(Ordering::SeqCst), iterations);

        shutdown_token.cancel();
    }
}
