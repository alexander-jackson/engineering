use std::time::Duration;

use chrono::Utc;
use color_eyre::eyre::Result;
use foundation_recurring_job::Job;
use reqwest::Client;
use sqlx::PgPool;
use tracing::{error, info};

mod tls;

use crate::certificate_checker::tls::check_certificate_expiry;

pub struct CertificateChecker {
    pool: PgPool,
    http_client: Client,
}

impl CertificateChecker {
    pub fn new(pool: PgPool, http_client: Client) -> Self {
        Self { pool, http_client }
    }
}

impl Job for CertificateChecker {
    const NAME: &'static str = "Certificate Checker";

    fn interval(&self) -> Duration {
        Duration::from_hours(24)
    }

    async fn run(&self) -> Result<()> {
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
