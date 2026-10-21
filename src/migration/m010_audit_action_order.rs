use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Read an action's newest entries without sorting its full history.
        manager
            .create_index(
                Index::create()
                    .name("idx_audit_log_action_order")
                    .table(Alias::new("audit_log"))
                    .col(Alias::new("action"))
                    .col(Alias::new("created_at"))
                    .col(Alias::new("id"))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_audit_log_action_order")
                    .table(Alias::new("audit_log"))
                    .to_owned(),
            )
            .await
    }
}
