use chrono::Utc;
use kellnr_common::account_name::UserName;
use kellnr_common::plain_token::PlainToken;
use kellnr_entity::{session, user};
use sea_orm::query::QuerySelect;
use sea_orm::sea_query::JoinType;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, ModelTrait, QueryFilter, RelationTrait, Set,
};

use super::{DB_DATE_FORMAT, Database};
use crate::error::DbError;
use crate::password::hash_token;
use crate::provider::{DbResult, SessionInfo};

impl Database {
    /// Timestamp (in [`DB_DATE_FORMAT`]) before which a session is expired, or
    /// `None` when sessions never expire. A session whose `created` value is
    /// older than the returned cutoff should not validate.
    fn session_expiry_cutoff(&self) -> Option<String> {
        self.session_age
            .map(|age| (Utc::now() - age).format(DB_DATE_FORMAT).to_string())
    }

    pub(super) async fn validate_session(
        &self,
        session_token: &PlainToken,
    ) -> DbResult<SessionInfo> {
        // An expired session is treated like a missing one: the age filter
        // makes it invisible here so callers get the same SessionNotFound.
        // With expiry disabled (no cutoff) the filter is omitted.
        let mut query = user::Entity::find()
            .join(JoinType::InnerJoin, user::Relation::Session.def())
            .filter(session::Column::Token.eq(hash_token(session_token.expose_secret())));
        if let Some(cutoff) = self.session_expiry_cutoff() {
            query = query.filter(session::Column::Created.gte(cutoff));
        }

        let u = query
            .one(&self.db_con)
            .await?
            .ok_or(DbError::SessionNotFound)?;

        Ok(SessionInfo {
            name: UserName::from_unchecked(u.name),
            is_admin: u.is_admin,
            is_read_only: u.is_read_only,
        })
    }

    pub(super) async fn delete_expired_sessions(&self) -> DbResult<u64> {
        // Expiry disabled: nothing to remove.
        let Some(cutoff) = self.session_expiry_cutoff() else {
            return Ok(0);
        };
        let res = session::Entity::delete_many()
            .filter(session::Column::Created.lt(cutoff))
            .exec(&self.db_con)
            .await?;
        Ok(res.rows_affected)
    }

    pub(super) async fn add_session_token(
        &self,
        name: &UserName,
        session_token: &PlainToken,
    ) -> DbResult<()> {
        let user = self.get_user(name).await?;
        let created = Utc::now().format(DB_DATE_FORMAT).to_string();

        let s = session::ActiveModel {
            token: Set(hash_token(session_token.expose_secret())),
            created: Set(created),
            user_fk: Set(user.id.get()),
            ..Default::default()
        };

        s.insert(&self.db_con).await?;
        Ok(())
    }

    pub(super) async fn delete_user_sessions(&self, user_name: &UserName) -> DbResult<u64> {
        let user = self.get_user_model(user_name).await?;
        let res = session::Entity::delete_many()
            .filter(session::Column::UserFk.eq(user.id))
            .exec(&self.db_con)
            .await?;
        Ok(res.rows_affected)
    }

    pub(super) async fn delete_other_user_sessions(
        &self,
        user_name: &UserName,
        keep_session_token: &PlainToken,
    ) -> DbResult<u64> {
        let user = self.get_user_model(user_name).await?;
        let res = session::Entity::delete_many()
            .filter(session::Column::UserFk.eq(user.id))
            .filter(session::Column::Token.ne(hash_token(keep_session_token.expose_secret())))
            .exec(&self.db_con)
            .await?;
        Ok(res.rows_affected)
    }

    pub(super) async fn delete_session_token(&self, session_token: &PlainToken) -> DbResult<()> {
        if let Some(s) = session::Entity::find()
            .filter(session::Column::Token.eq(hash_token(session_token.expose_secret())))
            .one(&self.db_con)
            .await?
        {
            s.delete(&self.db_con).await?;
        }

        Ok(())
    }
}
