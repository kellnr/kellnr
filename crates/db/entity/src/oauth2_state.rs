//! `SeaORM` Entity for `OAuth2` state (PKCE/CSRF storage during auth flow)

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "oauth2_state")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(column_type = "Text", unique)]
    pub state: String,
    #[sea_orm(column_type = "Text")]
    pub pkce_verifier: String,
    #[sea_orm(column_type = "Text")]
    pub nonce: String,
    #[sea_orm(column_type = "Text")]
    pub created: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
