use chrono::Utc;
use color_eyre::eyre::Result;
use foundation_shutdown::CancellationToken;
use reqwest::Client;
use sqlx::PgPool;
use tokio::time::{Duration, interval};
use tracing::{error, info};

mod tls;

use tls::check_certificate_expiry;

pub struct CertificateChecker {
    pool: PgPool,
    http_client: Client,
}

impl CertificateChecker {
    pub fn new(pool: PgPool, http_client: Client) -> Self {
        Self { pool, http_client }
    }

    pub async fn run(self, shutdown: CancellationToken) {
        let mut ticker = interval(Duration::from_secs(86400)); // 24 hours

        loop {
            tokio::select! {
                _ = shutdown.cancelled() => {
                    info!("certificate checker shutting down gracefully");
                    break;
                }
                _ = ticker.tick() => {
                    info!("starting certificate expiry check");

                    if let Err(e) = self.check_all_certificates().await {
                        error!("failed to check certificates: {}", e);
                    }
                }
            }
        }
    }

    async fn check_all_certificates(&self) -> Result<()> {
        let origins = crate::persistence::fetch_origins(&self.pool).await?;

        for origin in origins {
            // Only check HTTPS URIs
            if !origin.uri.starts_with("https://") {
                continue;
            }

            match check_certificate_expiry(&self.http_client, &origin.uri).await {
                Ok(expires_at) => {
                    info!(
                        origin_uri = %origin.uri,
                        expires_at = %expires_at,
                        "certificate expires"
                    );

                    if let Err(e) = crate::persistence::insert_certificate_check(
                        &self.pool,
                        origin.origin_uid,
                        expires_at,
                        Utc::now(),
                    )
                    .await
                    {
                        error!(
                            origin_uri = %origin.uri,
                            error = %e,
                            "failed to persist certificate check"
                        );
                    }
                }
                Err(e) => {
                    error!(
                        origin_uri = %origin.uri,
                        error = %e,
                        "failed to check certificate"
                    );
                }
            }
        }

        Ok(())
    }
}
