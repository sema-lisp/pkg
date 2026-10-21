use crate::{api_docs::ApiDocument, entity::package_api_doc};
use sea_orm::{ActiveModelTrait, ConnectionTrait, DbErr, EntityTrait, Set};

pub async fn find<C: ConnectionTrait>(
    db: &C,
    version_id: i64,
) -> Result<Option<ApiDocument>, DbErr> {
    package_api_doc::Entity::find_by_id(version_id)
        .one(db)
        .await?
        .map(|row| {
            serde_json::from_str(&row.document_json)
                .map_err(|error| DbErr::Custom(format!("Invalid API document: {error}")))
        })
        .transpose()
}

/// Insert once with the release transaction. Documents are never refreshed by
/// reads or by the historical backfill.
pub async fn insert<C: ConnectionTrait>(
    db: &C,
    version_id: i64,
    document: &ApiDocument,
) -> Result<(), DbErr> {
    package_api_doc::ActiveModel {
        version_id: Set(version_id),
        document_json: Set(
            serde_json::to_string(document).map_err(|error| DbErr::Custom(error.to_string()))?
        ),
        generated_at: Set(super::time::now()),
    }
    .insert(db)
    .await?;
    Ok(())
}
