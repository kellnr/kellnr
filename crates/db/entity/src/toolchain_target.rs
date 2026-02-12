//! `SeaORM` Entity for toolchain target archives

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "toolchain_target")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub toolchain_fk: i64,
    #[sea_orm(column_type = "Text")]
    pub target: String,
    #[sea_orm(column_type = "Text")]
    pub storage_path: String,
    #[sea_orm(column_type = "Text")]
    pub hash: String,
    pub size: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::toolchain::Entity",
        from = "Column::ToolchainFk",
        to = "super::toolchain::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Toolchain,
}

impl Related<super::toolchain::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Toolchain.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
