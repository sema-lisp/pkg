//! GitHub OAuth connection storage (encrypted access tokens).
//!
//! The upsert uses SeaORM's `on_conflict` so it lowers to each backend's
//! dialect rather than the SQLite/Postgres-only `INSERT ... ON CONFLICT ...
//! excluded.*` form.

use sea_orm::sea_query::{Expr, OnConflict};
use sea_orm::{ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, Set};

use crate::dal::time;
use crate::entity::oauth_connection;

const PROVIDER: &str = "github";

/// Insert or refresh a user's GitHub connection. On an existing
/// `(user_id, provider)` row this refreshes the provider identity, token, and
/// scopes, clears any prior revocation, and bumps `updated_at`.
pub async fn upsert_connection<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
    provider_user_id: &str,
    provider_login: &str,
    access_token_enc: Vec<u8>,
    scopes: &str,
) -> Result<(), DbErr> {
    let now = time::now();
    let row = oauth_connection::ActiveModel {
        user_id: Set(user_id),
        provider: Set(PROVIDER.to_string()),
        provider_user_id: Set(provider_user_id.to_string()),
        provider_login: Set(Some(provider_login.to_string())),
        access_token_enc: Set(access_token_enc),
        scopes: Set(Some(scopes.to_string())),
        revoked_at: Set(None),
        created_at: Set(now.clone()),
        updated_at: Set(Some(now)),
        ..Default::default()
    };

    oauth_connection::Entity::insert(row)
        .on_conflict(
            OnConflict::columns([
                oauth_connection::Column::UserId,
                oauth_connection::Column::Provider,
            ])
            .update_columns([
                oauth_connection::Column::ProviderUserId,
                oauth_connection::Column::ProviderLogin,
                oauth_connection::Column::AccessTokenEnc,
                oauth_connection::Column::Scopes,
                oauth_connection::Column::UpdatedAt,
            ])
            .value(
                oauth_connection::Column::RevokedAt,
                Expr::value(Option::<String>::None),
            )
            .to_owned(),
        )
        .exec(db)
        .await
        .map(|_| ())
}

/// The user's active (non-revoked) GitHub connection, if any.
pub async fn find_active<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
) -> Result<Option<oauth_connection::Model>, DbErr> {
    oauth_connection::Entity::find()
        .filter(oauth_connection::Column::UserId.eq(user_id))
        .filter(oauth_connection::Column::Provider.eq(PROVIDER))
        .filter(oauth_connection::Column::RevokedAt.is_null())
        .one(db)
        .await
}

/// Find a GitHub connection by the GitHub-side user id (used to detect a
/// GitHub account already linked to a different local user).
pub async fn find_by_provider_user_id<C: ConnectionTrait>(
    db: &C,
    provider_user_id: &str,
) -> Result<Option<oauth_connection::Model>, DbErr> {
    oauth_connection::Entity::find()
        .filter(oauth_connection::Column::Provider.eq(PROVIDER))
        .filter(oauth_connection::Column::ProviderUserId.eq(provider_user_id))
        .one(db)
        .await
}

/// Mark a user's GitHub connection revoked (e.g. after a 401 from GitHub).
pub async fn mark_revoked<C: ConnectionTrait>(db: &C, user_id: i64) -> Result<(), DbErr> {
    oauth_connection::Entity::update_many()
        .col_expr(
            oauth_connection::Column::RevokedAt,
            Expr::value(time::now()),
        )
        .filter(oauth_connection::Column::UserId.eq(user_id))
        .filter(oauth_connection::Column::Provider.eq(PROVIDER))
        .exec(db)
        .await
        .map(|_| ())
}
