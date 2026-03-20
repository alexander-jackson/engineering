use std::time::Duration;

use color_eyre::eyre::Result;
use foundation_shutdown::{CancellationToken, GracefulTask};
use sqlx::PgPool;
use sqlx::types::chrono::Utc;

use crate::renewal::Renewer;

pub struct Watcher {
    renewer: Renewer,
    pool: PgPool,
}

impl Watcher {
    pub fn new(renewer: Renewer, pool: PgPool) -> Self {
        Self { renewer, pool }
    }

    async fn poll(&self) -> Result<()> {
        // Find the certificate next expiring
        let expiries = crate::persistence::select_latest_expiry_per_domain(&self.pool).await?;

        tracing::info!(?expiries, "Current certificate expiries");

        let Some(next_expiry) = expiries.first() else {
            tracing::info!("No certificates found, sleeping for 24 hours...");
            tokio::time::sleep(Duration::from_hours(24)).await;
            return Ok(());
        };

        let now = Utc::now();
        let buffer = Duration::from_hours(24 * 7);

        if now > next_expiry.expires_at {
            tracing::warn!(
                domain = %next_expiry.domain,
                expires_at = %next_expiry.expires_at,
                "Certificate already expired, should have been renewed by now!"
            );

            return Ok(());
        }

        let time_until_expiry = next_expiry.expires_at - now;
        let buffer_time = now + buffer;

        let sleep_duration = if buffer_time < next_expiry.expires_at {
            Some(time_until_expiry.to_std()? - buffer)
        } else {
            None
        };

        tracing::info!(
            domain = %next_expiry.domain,
            expires_at = %next_expiry.expires_at,
            sleep_duration = ?sleep_duration,
            "Next certificate expiry, sleeping until then..."
        );

        if let Some(sleep_duration) = sleep_duration {
            tokio::time::sleep(sleep_duration).await;
        }

        // Time to renew the certificate!
        tracing::info!(
            domain = %next_expiry.domain,
            expires_at = %next_expiry.expires_at,
            "Time to renew certificate!"
        );

        let mut tx = self.pool.begin().await?;
        let expires_at = self.renewer.renew(&next_expiry.domain).await?;

        let certificate_uid = crate::persistence::insert_certificate(
            &mut tx,
            next_expiry.domain_uid,
            Utc::now(),
            expires_at,
        )
        .await?;

        tx.commit().await?;

        tracing::info!(
            domain = %next_expiry.domain,
            expires_at = %expires_at,
            %certificate_uid,
            "Certificate renewed and stored in database"
        );

        Ok(())
    }
}

impl GracefulTask for Watcher {
    async fn run_until_shutdown(self, shutdown: CancellationToken) -> Result<()> {
        loop {
            tokio::select! {
                _ = shutdown.cancelled() => {
                    tracing::info!("Watcher received shutdown signal, exiting...");
                    break;
                }
                result = self.poll() => {
                    if let Err(e) = result {
                        tracing::error!(?e, "Error while polling for certificate expiries");
                    }
                }
            }
        }

        Ok(())
    }
}
