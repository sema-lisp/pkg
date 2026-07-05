use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use std::sync::Arc;

use super::ApiError;
use crate::{
    auth::{generate_token, hash_token, AuthUser},
    AppState,
};

#[derive(Deserialize)]
pub struct CreateTokenRequest {
    pub name: String,
}

pub async fn create(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Json(body): Json<CreateTokenRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let token = generate_token();
    let token_hash = hash_token(&token);

    let model = crate::dal::tokens::create(&state.db, user.id, &body.name, &token_hash)
        .await
        .map_err(|_| ApiError::internal("Failed to create token"))?;

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "token": token,
            "id": model.id,
            "name": body.name,
        })),
    ))
}

pub async fn list(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
) -> impl IntoResponse {
    let rows = crate::dal::tokens::list_active_for_user(&state.db, user.id).await;

    let tokens: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| {
            serde_json::json!({
                "id": r.id,
                "name": r.name,
                "scopes": r.scopes,
                "created_at": r.created_at,
                "last_used_at": r.last_used_at,
            })
        })
        .collect();

    Json(serde_json::json!({"tokens": tokens}))
}

pub async fn revoke(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(token_id): Path<i64>,
) -> Result<impl IntoResponse, ApiError> {
    let rows_affected = crate::dal::tokens::revoke(&state.db, token_id, user.id).await;

    if rows_affected > 0 {
        Ok((StatusCode::OK, Json(serde_json::json!({"ok": true}))))
    } else {
        Err(ApiError::not_found("Token not found"))
    }
}
