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
                        tracing::warn!(job = %self.name, error = %e, "job execution failed");
                    }
                }
            }
        }

        Ok(())
    }
}
