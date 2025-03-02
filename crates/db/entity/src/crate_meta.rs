//! `SeaORM` Entity, @generated by sea-orm-codegen 1.1.5

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "crate_meta")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(column_type = "Text")]
    pub version: String,
    #[sea_orm(column_type = "Text")]
    pub created: String,
    pub downloads: i64,
    pub crate_fk: i64,
    #[sea_orm(column_type = "Text", nullable)]
    pub readme: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub license: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub license_file: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub documentation: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::krate::Entity",
        from = "Column::CrateFk",
        to = "super::krate::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Krate,
}

impl Related<super::krate::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Krate.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
