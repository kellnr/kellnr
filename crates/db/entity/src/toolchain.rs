//! `SeaORM` Entity for toolchain releases

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "toolchain")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(column_type = "Text")]
    pub name: String,
    #[sea_orm(column_type = "Text")]
    pub version: String,
    #[sea_orm(column_type = "Text")]
    pub date: String,
    #[sea_orm(column_type = "Text", nullable)]
    pub channel: Option<String>,
    #[sea_orm(column_type = "Text")]
    pub created: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::toolchain_target::Entity")]
    ToolchainTargets,
}

impl Related<super::toolchain_target::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ToolchainTargets.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
