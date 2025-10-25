use color_eyre::eyre::Result;
use tokio::signal::unix::SignalKind;
use tokio::sync::broadcast::{Receiver, Sender};

pub struct ShutdownCoordinator {
    sender: Sender<()>,
}

impl ShutdownCoordinator {
    pub fn new() -> Self {
        let (sender, _) = tokio::sync::broadcast::channel(1);

        Self { sender }
    }

    pub fn subscribe(&self) -> Receiver<()> {
        self.sender.subscribe()
    }

    pub async fn spawn(&self) -> Result<()> {
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

        let _ = self.sender.send(());

        Ok(())
    }
}

impl Default for ShutdownCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {}
