use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use color_eyre::eyre::Result;
use foundation_shutdown::{CancellationToken, GracefulTask};

pub struct RecurringJob<T, F>
where
    T: Send + 'static,
    F: for<'a> Fn(&'a T) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> + Send + 'static,
{
    name: String,
    interval: Duration,
    state: T,
    job: F,
}

impl<T, F> RecurringJob<T, F>
where
    T: Send + 'static,
    F: for<'a> Fn(&'a T) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> + Send + 'static,
{
    pub fn new(name: impl Into<String>, interval: Duration, state: T, job: F) -> Self {
        Self {
            name: name.into(),
            interval,
            state,
            job,
        }
    }
}

impl<T, F> GracefulTask for RecurringJob<T, F>
where
    T: Send + 'static,
    F: for<'a> Fn(&'a T) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> + Send + 'static,
{
    async fn run_until_shutdown(self, shutdown: CancellationToken) -> Result<()> {
        let mut interval = tokio::time::interval(self.interval);

        loop {
            tokio::select! {
                _ = shutdown.cancelled() => {
                    tracing::info!(job = %self.name, "shutting down gracefully");
                    break;
                }
                _ = interval.tick() => {
                    if let Err(e) = (self.job)(&self.state).await {
                        tracing::warn!(job = %self.name, error = ?e, "job execution failed");
                    }
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    use foundation_shutdown::{CancellationToken, GracefulTask};
    use tokio::sync::Mutex;
    use tokio::sync::mpsc::{Receiver, Sender};

    use crate::RecurringJob;

    struct State {
        counter: AtomicUsize,
        sender: Sender<()>,
        receiver: Mutex<Receiver<()>>,
    }

    #[tokio::test]
    async fn can_create_and_run_recurring_job() {
        let counter = AtomicUsize::new(0);

        let (test_sender, job_receiver) = tokio::sync::mpsc::channel(1);
        let (job_sender, mut test_receiver) = tokio::sync::mpsc::channel(1);

        let state = Arc::new(State {
            counter,
            sender: job_sender,
            receiver: Mutex::new(job_receiver),
        });

        let job = RecurringJob::new(
            "test-job",
            Duration::from_millis(1),
            Arc::clone(&state),
            |state| {
                Box::pin(async move {
                    // wait for a message
                    state.receiver.lock().await.recv().await;

                    // do some work
                    state.counter.fetch_add(1, Ordering::SeqCst);

                    // notify the test that the job has been run
                    state.sender.send(()).await.unwrap();

                    Ok(())
                })
            },
        );

        let shutdown_token = CancellationToken::new();
        let job_token = shutdown_token.clone();

        tokio::spawn(async move {
            job.run_until_shutdown(job_token).await.unwrap();
        });

        // trigger the job a couple of times
        let iterations = 3;

        for _ in 0..iterations {
            test_sender.send(()).await.unwrap();
            test_receiver.recv().await.unwrap();
        }

        // check the counter has been incremented the expected number of times
        assert_eq!(state.counter.load(Ordering::SeqCst), iterations);

        shutdown_token.cancel();
    }
}
