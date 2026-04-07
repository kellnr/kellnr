use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "toolchain_component")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub toolchain_target_fk: i64,
    #[sea_orm(column_type = "Text")]
    pub name: String,
    #[sea_orm(column_type = "Text")]
    pub storage_path: String,
    #[sea_orm(column_type = "Text")]
    pub hash: String,
    pub size: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::toolchain_target::Entity",
        from = "Column::ToolchainTargetFk",
        to = "super::toolchain_target::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    ToolchainTarget,
}

impl Related<super::toolchain_target::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ToolchainTarget.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
