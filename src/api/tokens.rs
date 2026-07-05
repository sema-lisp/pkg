use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use sea_orm::prelude::Expr;
use sea_orm::*;
use serde::Deserialize;
use std::sync::Arc;

use super::ApiError;
use crate::{
    auth::{generate_token, hash_token, AuthUser},
    entity::api_token,
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

    let new_token = api_token::ActiveModel {
        user_id: Set(user.id),
        name: Set(body.name.clone()),
        token_hash: Set(token_hash),
        created_at: Set(crate::dal::time::now()),
        ..Default::default()
    };

    let model = new_token
        .insert(&state.db)
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
    let rows = api_token::Entity::find()
        .filter(api_token::Column::UserId.eq(user.id))
        .filter(api_token::Column::RevokedAt.is_null())
        .order_by_desc(api_token::Column::CreatedAt)
        .all(&state.db)
        .await
        .unwrap_or_default();

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
    let result = api_token::Entity::update_many()
        .col_expr(
            api_token::Column::RevokedAt,
            Expr::value(crate::dal::time::now()),
        )
        .filter(api_token::Column::Id.eq(token_id))
        .filter(api_token::Column::UserId.eq(user.id))
        .filter(api_token::Column::RevokedAt.is_null())
        .exec(&state.db)
        .await;

    match result {
        Ok(r) if r.rows_affected > 0 => Ok((StatusCode::OK, Json(serde_json::json!({"ok": true})))),
        _ => Err(ApiError::not_found("Token not found")),
    }
}
