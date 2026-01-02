use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::extract::FromRequestParts;
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation};
use once_cell::sync::Lazy;
use uuid::Uuid;

use crate::error::ServerError;

static KEY: Lazy<String> = Lazy::new(|| std::env::var("JWT_KEY").unwrap());

fn get_encoding_key() -> EncodingKey {
    EncodingKey::from_secret(KEY.as_bytes())
}

fn get_decoding_key() -> DecodingKey {
    DecodingKey::from_secret(KEY.as_bytes())
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct Claims {
    pub id: Uuid,
    exp: u64,
}

impl Claims {
    /// Creates a JWT for the given user identifier with a 2 week duration of usage.
    pub fn create_token(id: Uuid) -> jsonwebtoken::errors::Result<String> {
        // Tokens default to lasting 2 weeks
        let duration = Duration::from_secs(2 * 7 * 24 * 60 * 60);

        Self::create_token_with_duration(id, duration)
    }

    /// Creates a JWT for the given user identifier with a specified duration of usage.
    ///
    /// This allows tokens to expire after the given time period, meaning the API server will
    /// reject them and force the user to login again.
    pub fn create_token_with_duration(
        id: Uuid,
        duration: Duration,
    ) -> jsonwebtoken::errors::Result<String> {
        let since_epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards");

        let exp = (since_epoch + duration).as_secs();

        let header = Header::default();
        let claims = Self { id, exp };
        let key = get_encoding_key();

        jsonwebtoken::encode(&header, &claims, &key)
    }

    pub fn from_authorization_header(value: Option<&str>) -> Result<Self, ServerError> {
        value
            .and_then(|v| v.strip_prefix("Bearer "))
            .and_then(|v| {
                let decoding_key = get_decoding_key();
                let validation = Validation::default();

                jsonwebtoken::decode::<Claims>(v, &decoding_key, &validation)
                    .ok()
                    .map(|v| v.claims)
            })
            .ok_or(ServerError::UNPROCESSABLE_ENTITY)
    }
}

impl<State: Sync> FromRequestParts<State> for Claims {
    type Rejection = ServerError;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &State,
    ) -> Result<Self, Self::Rejection> {
        let authorization = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok());

        Self::from_authorization_header(authorization)
    }
}
