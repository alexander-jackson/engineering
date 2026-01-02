use std::env;

use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use uuid::Uuid;

use crate::error::ServerError;

/// The configuration for sending emails.
#[derive(Debug)]
pub struct Config {
    /// The address to send from.
    pub from_address: String,
    /// The name to send from.
    pub from_name: String,
    /// The application specific password for Gmail.
    pub app_password: String,
}

impl Config {
    /// Builds a configuration from the environment variables.
    pub fn from_env() -> Result<Self, std::env::VarError> {
        Ok(Self {
            from_address: env::var("FROM_ADDRESS")?,
            from_name: env::var("FROM_NAME")?,
            app_password: env::var("APP_PASSWORD")?,
        })
    }
}

/// Sends an email to the user confirming their booking for a given session.
pub async fn send_verification_email(
    email_address: &str,
    email_address_uid: Uuid,
) -> Result<(), ServerError> {
    // Check whether email settings are on
    if env::var("SEND_EMAILS").is_err() {
        return Ok(());
    }

    let config = Config::from_env().expect("Config was malformed");

    let prefix = email_address.split('@').next().unwrap();

    let from = format!("{} <{}>", config.from_name, config.from_address);
    let to = format!("{} <{}>", prefix, email_address);
    let body = format!(
        r#"Hey,
We need to verify your email address. Please click the following link:

http://localhost:3000/verify-email/{}"#,
        email_address_uid
    );

    let email = Message::builder()
        .from(from.parse().unwrap())
        .to(to.parse().unwrap())
        .subject("OpenTracker Email Verification")
        .body(body)
        .unwrap();

    let creds = Credentials::new(config.from_address, config.app_password);

    // Open a remote connection to gmail
    let mailer = AsyncSmtpTransport::<Tokio1Executor>::relay("smtp.gmail.com")
        .unwrap()
        .credentials(creds)
        .build();

    mailer
        .send(email)
        .await
        .expect("Failed to send an email to the client");

    tracing::info!(%email_address_uid, "Sent an email to the client");

    Ok(())
}
