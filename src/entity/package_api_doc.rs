use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "package_api_docs")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub version_id: i64,
    pub document_json: String,
    pub generated_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::package_version::Entity",
        from = "Column::VersionId",
        to = "super::package_version::Column::Id"
    )]
    Version,
}
impl ActiveModelBehavior for ActiveModel {}
