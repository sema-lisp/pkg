//! User lookups needed by other aggregates (kept intentionally minimal).

use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, ConnectionTrait, DbErr, EntityTrait, QueryFilter, Set,
};

use crate::dal::time;
use crate::entity::user;

/// Look up a user by their unique username.
pub async fn find_by_username<C: ConnectionTrait>(
    db: &C,
    username: &str,
) -> Result<Option<user::Model>, DbErr> {
    user::Entity::find()
        .filter(user::Column::Username.eq(username))
        .one(db)
        .await
}

/// Create a new user with a password hash, stamping `created_at` in Rust.
/// Uniqueness of `username`/`email` is enforced by the table's unique indexes,
/// so a duplicate surfaces as a `DbErr` from the insert.
pub async fn create<C: ConnectionTrait>(
    db: &C,
    username: &str,
    email: &str,
    password_hash: &str,
) -> Result<user::Model, DbErr> {
    let new_user = user::ActiveModel {
        username: Set(username.to_string()),
        email: Set(email.to_string()),
        password_hash: Set(Some(password_hash.to_string())),
        created_at: Set(time::now()),
        ..Default::default()
    };
    new_user.insert(db).await
}

/// Look up a user by either their username or email (used at login, where the
/// same input field accepts both).
pub async fn find_by_username_or_email<C: ConnectionTrait>(
    db: &C,
    login: &str,
) -> Result<Option<user::Model>, DbErr> {
    user::Entity::find()
        .filter(
            Condition::any()
                .add(user::Column::Username.eq(login))
                .add(user::Column::Email.eq(login)),
        )
        .one(db)
        .await
}

/// Look up a user by id, excluding banned accounts (`banned_at` set).
pub async fn find_active_by_id<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
) -> Result<Option<user::Model>, DbErr> {
    user::Entity::find_by_id(user_id)
        .filter(user::Column::BannedAt.is_null())
        .one(db)
        .await
}
