use axum::extract::Path;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use uuid::Uuid;

use crate::auth::Claims;
use crate::email;
use crate::endpoints::State;
use crate::error::{ServerError, ServerResponse};
use crate::forms;
use crate::persistence::{self};

pub fn router(state: State) -> Router {
    Router::new()
        .route("/login", put(login))
        .route("/register", put(register))
        .route("/email/status", get(get_email_verification_status))
        .route("/email/verify/resend", post(send_verification_email))
        .route("/email/verify/{email_address_uid}", put(verify_email))
        .route("/profile/update-password", post(update_password))
        .with_state(state)
}

pub async fn register(
    axum::extract::State(State { pool }): axum::extract::State<State>,
    Json(registration): Json<forms::Registration>,
) -> ServerResponse<Json<Option<String>>> {
    // Check whether they are unique
    let contents = persistence::account::find_by_email(&registration.email, &pool).await?;

    if contents.is_some() {
        tracing::warn!("User tried to register with an existing email address");
        return Err(ServerError::CONFLICT.with_message("User already exists with that email"));
    }

    tracing::info!("Registering a new user");

    let hashed = bcrypt::hash(registration.password, bcrypt::DEFAULT_COST)?;

    // Create a unique identifier for the user
    let user_id = persistence::account::insert(&registration.email, &hashed, &pool).await?;

    let claims = Claims::create_token(user_id)?;

    Ok(Json(Some(claims)))
}

pub async fn login(
    axum::extract::State(State { pool }): axum::extract::State<State>,
    Json(login): Json<forms::Login>,
) -> ServerResponse<Json<Option<String>>> {
    // Get users with the same email
    let account = persistence::account::find_by_email(&login.email, &pool)
        .await?
        .ok_or_else(|| {
            tracing::warn!("Failed to find user in database");
            ServerError::NOT_FOUND.with_message("No user found for email")
        })?;

    if !bcrypt::verify(login.password, &account.password).unwrap_or(false) {
        tracing::warn!("Password failed to match the database");
        return Err(ServerError::UNAUTHORIZED.with_message("Authorisation failed"));
    }

    let claims = Claims::create_token(account.account_uid)?;

    Ok(Json(Some(claims)))
}

pub async fn update_password(
    axum::extract::State(State { pool }): axum::extract::State<State>,
    claims: Claims,
    Json(payload): Json<forms::UpdatePassword>,
) -> ServerResponse<()> {
    tracing::debug!(?payload, "Attempting to update a password");

    // Check the updating passwords are the same
    if payload.new_password != payload.repeat_password {
        tracing::warn!("Provided passwords did not match");
        return Err(ServerError::UNPROCESSABLE_ENTITY);
    }

    // Fetch the current user information
    let user = persistence::account::find_by_id(claims.id, &pool)
        .await?
        .ok_or(ServerError::UNAUTHORIZED)?;

    // Validate their password
    if !bcrypt::verify(payload.current_password, &user.password)? {
        tracing::warn!("Existing password did not match what was stored in the database");
        return Err(ServerError::UNAUTHORIZED);
    }

    // Passwords matched, so update to the new one
    let hashed = bcrypt::hash(payload.new_password, bcrypt::DEFAULT_COST)?;

    tracing::info!("Updating the user's password");

    persistence::account::update_password(claims.id, &hashed, &pool).await?;

    Ok(())
}

pub async fn get_email_verification_status(
    claims: Claims,
    axum::extract::State(State { pool }): axum::extract::State<State>,
) -> ServerResponse<Json<persistence::account::EmailVerificationStatus>> {
    let status = persistence::account::fetch_email_verification_status(claims.id, &pool).await?;

    tracing::debug!(?status.verified_at, "Checking if email address is verified");

    Ok(Json(status))
}

pub async fn send_verification_email(
    claims: Claims,
    axum::extract::State(State { pool }): axum::extract::State<State>,
) -> ServerResponse<()> {
    // Fetch the currently pending email address if it exists
    let status = persistence::account::fetch_email_verification_status(claims.id, &pool).await?;

    email::send_verification_email(&status.email_address, status.email_address_uid).await?;

    Ok(())
}

pub async fn verify_email(
    Path(email_address_uid): Path<Uuid>,
    axum::extract::State(State { pool }): axum::extract::State<State>,
) -> ServerResponse<()> {
    tracing::info!(%email_address_uid, "Verifying email address for user");

    persistence::account::verify_email(email_address_uid, &pool).await?;

    Ok(())
}
