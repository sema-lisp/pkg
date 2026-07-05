use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use sea_orm::*;
use serde::Deserialize;
use std::sync::Arc;

use super::ApiError;
use crate::{
    auth::{
        clear_session_cookie, cookie_secure, create_session, delete_session, hash_password,
        session_cookie, validate_email, validate_password, validate_username, verify_password,
    },
    entity::user,
    AppState,
};

/// Read the `session=` value out of a Cookie header, if present.
fn session_from_cookies(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())?
        .split(';')
        .filter_map(|c| c.trim().strip_prefix("session="))
        .next()
        .map(|s| s.to_string())
}

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub email: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

pub async fn register(
    State(state): State<Arc<AppState>>,
    Json(body): Json<RegisterRequest>,
) -> Result<impl IntoResponse, ApiError> {
    validate_username(&body.username).map_err(ApiError::bad_request)?;
    validate_email(&body.email).map_err(ApiError::bad_request)?;
    validate_password(&body.password).map_err(ApiError::bad_request)?;

    let username = body.username.to_lowercase();
    let email = body.email.to_lowercase();

    let password_hash = hash_password(&body.password);

    let new_user = user::ActiveModel {
        username: Set(username.clone()),
        email: Set(email),
        password_hash: Set(Some(password_hash)),
        ..Default::default()
    };

    let model = new_user
        .insert(&state.db)
        .await
        .map_err(|_| ApiError::conflict("Registration failed"))?;

    let user_id = model.id;
    let session_id = create_session(&state.db, user_id)
        .await
        .map_err(|_| ApiError::internal("Failed to create session"))?;
    crate::audit::log(
        &state.db,
        &username,
        "register",
        Some("user"),
        Some(&username),
        None,
    )
    .await;
    let secure = cookie_secure(&state.config.base_url);
    Ok((
        StatusCode::CREATED,
        [(header::SET_COOKIE, session_cookie(&session_id, secure))],
        Json(serde_json::json!({"ok": true, "username": username})),
    ))
}

pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(body): Json<LoginRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let login_input = body.username.to_lowercase();

    let row = user::Entity::find()
        .filter(
            Condition::any()
                .add(user::Column::Username.eq(&login_input))
                .add(user::Column::Email.eq(&login_input)),
        )
        .one(&state.db)
        .await
        .ok()
        .flatten()
        .ok_or_else(|| ApiError::new(StatusCode::UNAUTHORIZED, "Invalid credentials"))?;

    let hash = row
        .password_hash
        .as_ref()
        .ok_or_else(|| ApiError::new(StatusCode::UNAUTHORIZED, "Account uses GitHub login only"))?;

    if !verify_password(&body.password, hash) {
        return Err(ApiError::new(
            StatusCode::UNAUTHORIZED,
            "Invalid credentials",
        ));
    }

    let session_id = create_session(&state.db, row.id)
        .await
        .map_err(|_| ApiError::internal("Failed to create session"))?;
    let secure = cookie_secure(&state.config.base_url);
    Ok((
        StatusCode::OK,
        [(header::SET_COOKIE, session_cookie(&session_id, secure))],
        Json(serde_json::json!({"ok": true, "username": row.username})),
    ))
}

pub async fn logout(State(state): State<Arc<AppState>>, headers: HeaderMap) -> impl IntoResponse {
    // Invalidate the session server-side so a captured cookie can't be
    // replayed after logout — clearing the cookie alone is not enough.
    if let Some(session_id) = session_from_cookies(&headers) {
        delete_session(&state.db, &session_id).await;
    }
    let secure = cookie_secure(&state.config.base_url);
    (
        StatusCode::OK,
        [(header::SET_COOKIE, clear_session_cookie(secure))],
        Json(serde_json::json!({"ok": true})),
    )
}
