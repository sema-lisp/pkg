use axum::{extract::State, response::IntoResponse, Json};
use serde::Deserialize;
use std::sync::Arc;

use super::ApiError;
use crate::{auth::AnyAuth, auth::AuthUser, AppState};

#[derive(Deserialize)]
pub struct UpdateProfileRequest {
    pub email: String,
    pub homepage: Option<String>,
}

/// Update the logged-in user's profile (email + optional homepage URL).
pub async fn update(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Json(body): Json<UpdateProfileRequest>,
) -> Result<impl IntoResponse, ApiError> {
    crate::auth::validate_email(&body.email).map_err(ApiError::bad_request)?;
    // Validate the homepage only when one is actually supplied; a blank value
    // clears it and should not trip URL validation.
    if let Some(hp) = body
        .homepage
        .as_deref()
        .map(str::trim)
        .filter(|h| !h.is_empty())
    {
        crate::auth::validate_homepage(hp).map_err(ApiError::bad_request)?;
    }
    crate::dal::users::update_profile(
        &state.db,
        user.id,
        &body.email.to_lowercase(),
        body.homepage.as_deref(),
    )
    .await
    .map_err(|_| ApiError::conflict("Could not update profile (email already in use?)"))?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

/// The authenticated account behind the presented credential — a session
/// cookie (web) or a Bearer API token (CLI).
pub async fn me(AnyAuth(user): AnyAuth) -> impl IntoResponse {
    Json(serde_json::json!({
        "id": user.id,
        "username": user.username,
        "email": user.email,
        "is_admin": user.is_admin,
        "is_official": crate::auth::is_official(&user.username),
    }))
}
