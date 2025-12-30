use std::future::Future;

use color_eyre::eyre::Result;
use tokio::signal::unix::SignalKind;
use tokio::task::JoinHandle;

pub use tokio_util::sync::CancellationToken;

pub trait GracefulTask: Send + 'static {
    fn run_until_shutdown(
        self,
        shutdown: CancellationToken,
    ) -> impl Future<Output = Result<()>> + Send;
}

pub struct ShutdownCoordinator {
    token: CancellationToken,
    handles: Vec<JoinHandle<Result<()>>>,
}

impl ShutdownCoordinator {
    pub fn new() -> Self {
        Self {
            token: CancellationToken::new(),
            handles: Vec::new(),
        }
    }

    pub fn token(&self) -> CancellationToken {
        self.token.clone()
    }

    pub fn spawn_task(&mut self, task: impl GracefulTask) -> JoinHandle<Result<()>> {
        let token = self.token();
        tokio::spawn(async move { task.run_until_shutdown(token).await })
    }

    /// Spawns a background task that listens for Ctrl+C and SIGTERM signals
    /// and cancels the coordinator's token when received.
    pub fn listen_for_signals(&self) {
        let token = self.token.clone();

        tokio::spawn(async move {
            let ctrl_c = async {
                tokio::signal::ctrl_c()
                    .await
                    .expect("failed to install Ctrl+C handler");

                tracing::info!("received ctrl+c, starting graceful shutdown");
            };

            let terminate = async {
                tokio::signal::unix::signal(SignalKind::terminate())
                    .expect("failed to install signal handler")
                    .recv()
                    .await;

                tracing::info!("received terminate signal, starting graceful shutdown");
            };

            tokio::select! {
                _ = ctrl_c => {},
                _ = terminate => {},
            }

            token.cancel();
        });
    }

    pub fn with_task<T>(mut self, task: T) -> Self
    where
        T: GracefulTask,
    {
        let token = self.token();
        let handle = tokio::spawn(async move { task.run_until_shutdown(token).await });
        self.handles.push(handle);
        self
    }

    pub async fn run(self) -> Result<()> {
        self.listen_for_signals();

        for handle in self.handles {
            handle.await??;
        }

        Ok(())
    }
}

impl Default for ShutdownCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use color_eyre::eyre::{Result, eyre};
    use tokio_util::sync::CancellationToken;

    use crate::{GracefulTask, ShutdownCoordinator};

    #[derive(Clone)]
    struct TestTask {
        counter: Arc<Mutex<u32>>,
    }

    impl GracefulTask for TestTask {
        async fn run_until_shutdown(self, shutdown: CancellationToken) -> Result<()> {
            loop {
                tokio::select! {
                    _ = shutdown.cancelled() => {
                        break;
                    }
                    _ = tokio::time::sleep(Duration::from_millis(10)) => {
                        let mut count = self.counter.lock().unwrap();
                        *count += 1;
                    }
                }
            }
            Ok(())
        }
    }

    #[tokio::test]
    async fn can_create_shutdown_coordinator() {
        let coordinator = ShutdownCoordinator::new();
        let token = coordinator.token();

        assert!(!token.is_cancelled());
    }

    #[tokio::test]
    async fn cancelling_token_stops_task() -> Result<()> {
        let mut coordinator = ShutdownCoordinator::new();
        let counter = Arc::new(Mutex::new(0));

        let task = TestTask {
            counter: Arc::clone(&counter),
        };

        let handle = coordinator.spawn_task(task);
        let token = coordinator.token();

        // Let the task run for a bit
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Cancel the token
        token.cancel();

        // Wait for task to finish
        handle.await??;

        // Verify the task ran at least once
        let count = *counter.lock().unwrap();
        assert!(count > 0);

        Ok(())
    }

    #[tokio::test]
    async fn can_coordinate_multiple_tasks() -> Result<()> {
        let mut coordinator = ShutdownCoordinator::new();
        let counter1 = Arc::new(Mutex::new(0));
        let counter2 = Arc::new(Mutex::new(0));

        let task1 = TestTask {
            counter: Arc::clone(&counter1),
        };
        let task2 = TestTask {
            counter: Arc::clone(&counter2),
        };

        let handle1 = coordinator.spawn_task(task1);
        let handle2 = coordinator.spawn_task(task2);
        let token = coordinator.token();

        // Let tasks run
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Cancel all tasks
        token.cancel();

        // Wait for all tasks
        handle1.await??;
        handle2.await??;

        // Both tasks should have run
        assert!(*counter1.lock().unwrap() > 0);
        assert!(*counter2.lock().unwrap() > 0);

        Ok(())
    }

    #[tokio::test]
    async fn builder_pattern_runs_multiple_tasks() -> Result<()> {
        let coordinator = ShutdownCoordinator::new();
        let token = coordinator.token();

        let counter1 = Arc::new(Mutex::new(0));
        let counter2 = Arc::new(Mutex::new(0));

        let task1 = TestTask {
            counter: Arc::clone(&counter1),
        };
        let task2 = TestTask {
            counter: Arc::clone(&counter2),
        };

        let coordinator = coordinator.with_task(task1).with_task(task2);

        // Spawn the coordinator in the background
        let coordinator_handle = tokio::spawn(async move { coordinator.run().await });

        // Let tasks run
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Cancel and wait
        token.cancel();
        coordinator_handle.await??;

        // Both tasks should have run
        assert!(*counter1.lock().unwrap() > 0);
        assert!(*counter2.lock().unwrap() > 0);

        Ok(())
    }

    struct FailingTask;

    impl GracefulTask for FailingTask {
        async fn run_until_shutdown(self, _shutdown: CancellationToken) -> Result<()> {
            tokio::time::sleep(Duration::from_millis(10)).await;
            Err(eyre!("task failed"))
        }
    }

    #[tokio::test]
    async fn task_errors_are_propagated() -> Result<()> {
        let mut coordinator = ShutdownCoordinator::new();
        let task = FailingTask;

        let handle = coordinator.spawn_task(task);
        let result = handle.await?;

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "task failed");

        Ok(())
    }

    #[tokio::test]
    async fn builder_pattern_propagates_errors() {
        let coordinator = ShutdownCoordinator::new();

        // Create two failing tasks so we don't hang waiting for one to complete
        let bad_task1 = FailingTask;
        let bad_task2 = FailingTask;

        let coordinator = coordinator.with_task(bad_task1).with_task(bad_task2);

        let result = coordinator.run().await;

        assert!(result.is_err());
    }

    struct ImmediateTask {
        completed: Arc<Mutex<bool>>,
    }

    impl GracefulTask for ImmediateTask {
        async fn run_until_shutdown(self, _shutdown: CancellationToken) -> Result<()> {
            *self.completed.lock().unwrap() = true;
            Ok(())
        }
    }

    #[tokio::test]
    async fn tasks_that_complete_immediately_work() -> Result<()> {
        let mut coordinator = ShutdownCoordinator::new();
        let completed = Arc::new(Mutex::new(false));

        let task = ImmediateTask {
            completed: Arc::clone(&completed),
        };

        let handle = coordinator.spawn_task(task);
        handle.await??;

        assert!(*completed.lock().unwrap());

        Ok(())
    }
}
