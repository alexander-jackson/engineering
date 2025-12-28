use color_eyre::eyre::Result;
use tokio::signal::unix::SignalKind;

pub use tokio_util::sync::CancellationToken;

pub struct ShutdownCoordinator {
    token: CancellationToken,
}

impl ShutdownCoordinator {
    pub fn new() -> Self {
        Self {
            token: CancellationToken::new(),
        }
    }

    pub fn token(&self) -> CancellationToken {
        self.token.clone()
    }

    pub async fn spawn(self) -> Result<()> {
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

        self.token.cancel();

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
