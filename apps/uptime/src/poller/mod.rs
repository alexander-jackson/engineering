use std::fmt::{self, Display};
use std::time::Duration;

use color_eyre::eyre::Result;
use foundation_recurring_job::Job;
use sqlx::PgPool;
use sqlx::types::chrono::Utc;
use uuid::Uuid;

use crate::persistence::{NotificationType, Origin};

#[derive(Copy, Clone, Debug, sqlx::Type)]
pub enum FailureReason {
    RequestTimeout,
    Redirection,
    BadRequest,
    ConnectionFailure,
    InvalidBody,
    Unknown,
}

impl FailureReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::RequestTimeout => "RequestTimeout",
            Self::Redirection => "Redirection",
            Self::BadRequest => "BadRequest",
            Self::ConnectionFailure => "ConnectionFailure",
            Self::InvalidBody => "InvalidBody",
            Self::Unknown => "Unknown",
        }
    }
}

impl Display for FailureReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let repr = self.as_str();

        write!(f, "{repr}")
    }
}

impl From<reqwest::Error> for FailureReason {
    fn from(error: reqwest::Error) -> Self {
        if error.is_timeout() {
            return Self::RequestTimeout;
        }

        if error.is_redirect() {
            return Self::Redirection;
        }

        if error.is_request() {
            return Self::BadRequest;
        }

        if error.is_connect() {
            return Self::ConnectionFailure;
        }

        if error.is_body() {
            return Self::InvalidBody;
        }

        Self::Unknown
    }
}

pub trait Notifier {
    fn notify(
        &self,
        topic: &str,
        subject: &str,
        message: &str,
    ) -> impl Future<Output = Result<()>> + Send;
}

impl Notifier for aws_sdk_sns::Client {
    async fn notify(&self, topic: &str, subject: &str, message: &str) -> Result<()> {
        self.publish()
            .topic_arn(topic)
            .subject(subject)
            .message(message)
            .send()
            .await?;

        Ok(())
    }
}

#[derive(Copy, Clone, Debug)]
pub struct AlertThreshold {
    /// The number of failures that need to occur for a notification to be sent.
    failure_limit: u16,
    /// The window where failures must have occurred.
    window_period: chrono::Duration,
    /// The minimum amount of time between notifications.
    cooldown: chrono::Duration,
}

impl Default for AlertThreshold {
    fn default() -> Self {
        Self {
            failure_limit: 3,
            window_period: chrono::Duration::minutes(5),
            cooldown: chrono::Duration::hours(1),
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct CertificateAlertThreshold {
    /// The minimum amount of time between certificate expiry notifications.
    cooldown: chrono::Duration,
}

impl Default for CertificateAlertThreshold {
    fn default() -> Self {
        Self {
            cooldown: chrono::Duration::days(3),
        }
    }
}

#[derive(Clone, Debug)]
pub struct PollerConfiguration {
    alert_threshold: AlertThreshold,
    certificate_alert_threshold: CertificateAlertThreshold,
    topic: String,
}

impl PollerConfiguration {
    pub fn new<T: Into<String>>(
        alert_threshold: AlertThreshold,
        certificate_alert_threshold: CertificateAlertThreshold,
        topic: T,
    ) -> Self {
        Self {
            alert_threshold,
            certificate_alert_threshold,
            topic: topic.into(),
        }
    }
}

pub struct Poller<N> {
    pool: PgPool,
    http_client: reqwest::Client,
    notifier: N,
    configuration: PollerConfiguration,
}

impl<N: Notifier> Poller<N> {
    pub fn new(
        pool: PgPool,
        http_client: reqwest::Client,
        notifier: N,
        configuration: PollerConfiguration,
    ) -> Self {
        Self {
            pool,
            http_client,
            notifier,
            configuration,
        }
    }

    async fn check_for_pending_notifications(&self, origin_uid: Uuid, uri: &str) -> Result<()> {
        self.check_for_certificate_expiry(origin_uid, uri).await?;
        self.check_for_outage(origin_uid, uri).await?;

        Ok(())
    }

    async fn check_for_certificate_expiry(&self, origin_uid: Uuid, uri: &str) -> Result<()> {
        let PollerConfiguration {
            certificate_alert_threshold,
            topic,
            ..
        } = &self.configuration;

        // Check for certificate expiry
        let certificate_expiry_threshold = chrono::Duration::days(30);

        let most_recent_check =
            crate::persistence::fetch_most_recent_certificate_check(&self.pool, origin_uid).await?;

        if let Some(check) = most_recent_check {
            if check.expires_at - Utc::now() > certificate_expiry_threshold {
                tracing::debug!(%origin_uid, expires_at = %check.expires_at, "certificate is not expiring soon");
                return Ok(());
            }

            let cooled_down = crate::persistence::latest_notification_older_than(
                &self.pool,
                origin_uid,
                NotificationType::CertificateExpiry,
                certificate_alert_threshold.cooldown,
            )
            .await?;

            if !cooled_down {
                tracing::debug!(%origin_uid, expires_at = %check.expires_at, "certificate is expiring soon, but a notification has been sent recently");
                return Ok(());
            }

            let now = Utc::now();
            let days_remaining = (check.expires_at - now).num_days();

            let subject = "Certificate expiring soon";
            let message = format!(
                "The TLS certificate for {uri} will expire in {} days (on {})",
                days_remaining,
                check.expires_at.format("%Y-%m-%d")
            );

            self.notifier.notify(topic, subject, &message).await?;

            let created_at = Utc::now();

            let notification_uid = crate::persistence::insert_notification(
                &self.pool,
                origin_uid,
                NotificationType::CertificateExpiry,
                topic,
                subject,
                &message,
                created_at,
            )
            .await?;

            tracing::info!(%origin_uid, %notification_uid, days_remaining, "routed certificate expiry notification");
        }

        Ok(())
    }

    async fn check_for_outage(&self, origin_uid: Uuid, uri: &str) -> Result<()> {
        let PollerConfiguration {
            alert_threshold,
            topic,
            ..
        } = &self.configuration;

        // Check for failure rate
        let exceeded = crate::persistence::failure_rate_exceeded(
            &self.pool,
            origin_uid,
            alert_threshold.failure_limit,
            alert_threshold.window_period,
        )
        .await?;

        if !exceeded {
            tracing::debug!(%origin_uid, ?alert_threshold, "failure rate has not been exceeded");
            return Ok(());
        }

        let cooled_down = crate::persistence::latest_notification_older_than(
            &self.pool,
            origin_uid,
            NotificationType::Uptime,
            alert_threshold.cooldown,
        )
        .await?;

        if !cooled_down {
            tracing::debug!(%origin_uid, ?alert_threshold, "failure rate is exceeded, but a notification has been sent recently");
            return Ok(());
        }

        let subject = "Outage detected";
        let message = format!("The failure rate of {uri} exceeds the SLA");

        self.notifier
            .notify(topic, "Outage detected", &message)
            .await?;

        let created_at = Utc::now();

        let notification_uid = crate::persistence::insert_notification(
            &self.pool,
            origin_uid,
            NotificationType::Uptime,
            topic,
            subject,
            &message,
            created_at,
        )
        .await?;

        tracing::info!(%origin_uid, %notification_uid, "routed a new notification");

        Ok(())
    }
}

impl<N: Notifier + Send + Sync + 'static> Job for Poller<N> {
    const NAME: &'static str = "Origin Poller";
    const INTERVAL: Duration = Duration::from_secs(60);

    async fn run(&self) -> Result<()> {
        let Self {
            pool, http_client, ..
        } = self;

        // Find all the available origins
        let origins = crate::persistence::fetch_origins(pool).await?;
        let timeout = Duration::from_secs(3);

        for Origin { origin_uid, uri } in origins {
            let mut tx = pool.begin().await?;
            let start = Utc::now();

            match http_client.get(&uri).timeout(timeout).send().await {
                Ok(res) => {
                    let status = res.status();
                    let latency_millis = (Utc::now() - start).num_milliseconds();

                    let query_uid = crate::persistence::insert_query(
                        &mut tx,
                        origin_uid,
                        status.as_u16(),
                        latency_millis,
                        start,
                    )
                    .await?;

                    tracing::info!(
                        %origin_uid,
                        %query_uid,
                        %status,
                        %latency_millis,
                        "made a request to the origin"
                    );
                }
                Err(e) => {
                    let failure_reason = FailureReason::from(e);

                    let query_failure_uid = crate::persistence::insert_query_failure(
                        &mut tx,
                        origin_uid,
                        failure_reason,
                        start,
                    )
                    .await?;

                    tracing::warn!(
                        %origin_uid,
                        %query_failure_uid,
                        %failure_reason,
                        "failed to make a request to the origin"
                    );
                }
            }

            tx.commit().await?;

            // Check whether we need to notify someone
            self.check_for_pending_notifications(origin_uid, &uri)
                .await?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests;
