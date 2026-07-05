//! Version dependency rows.

use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, Set,
};

use crate::entity::dependency;

/// All dependency edges recorded for a version.
pub async fn list_for_version<C: ConnectionTrait>(
    db: &C,
    version_id: i64,
) -> Vec<dependency::Model> {
    dependency::Entity::find()
        .filter(dependency::Column::VersionId.eq(version_id))
        .all(db)
        .await
        .unwrap_or_default()
}

/// Record a single dependency edge for a published version.
pub async fn insert<C: ConnectionTrait>(
    db: &C,
    version_id: i64,
    name: &str,
    version_req: &str,
) -> Result<(), DbErr> {
    let row = dependency::ActiveModel {
        version_id: Set(version_id),
        dependency_name: Set(name.to_string()),
        version_req: Set(version_req.to_string()),
        ..Default::default()
    };
    row.insert(db).await.map(|_| ())
}
