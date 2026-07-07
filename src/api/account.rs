use axum::{extract::State, response::IntoResponse, Json};
use serde::Deserialize;
use std::sync::Arc;

use super::ApiError;
use crate::{auth::AuthUser, AppState};

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
    if let Some(hp) = body.homepage.as_deref().map(str::trim).filter(|h| !h.is_empty()) {
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
