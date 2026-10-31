use chrono::Utc;
use kellnr_common::account_name::UserName;
use kellnr_common::plain_token::PlainToken;
use kellnr_entity::{auth_token, user};
use kellnr_migration::iden::AuthTokenIden;
use sea_orm::query::QuerySelect;
use sea_orm::sea_query::{Expr, JoinType};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, ExprTrait, QueryFilter, RelationTrait, Set,
};
use tracing::warn;

use super::{DB_DATE_FORMAT, Database};
use crate::error::DbError;
use crate::password::hash_token;
use crate::provider::DbResult;
use crate::{AuthToken, AuthTokenId, User};

/// How often the last use of an auth token is written at most.
const TOKEN_LAST_USED_INTERVAL: chrono::TimeDelta = chrono::TimeDelta::hours(1);

impl Database {
    pub(super) async fn add_auth_token(
        &self,
        name: &str,
        token: &PlainToken,
        user: &UserName,
    ) -> DbResult<()> {
        let hashed_token = hash_token(token.expose_secret());
        let user = self.get_user_model(user).await?;

        let at = auth_token::ActiveModel {
            name: Set(name.to_owned()),
            token: Set(hashed_token),
            user_fk: Set(user.id),
            created: Set(Some(Utc::now().format(DB_DATE_FORMAT).to_string())),
            ..Default::default()
        };

        at.insert(&self.db_con).await?;

        Ok(())
    }

    pub(super) async fn get_user_from_token(&self, token: &PlainToken) -> DbResult<User> {
        let token = hash_token(token.expose_secret());

        let (at, u) = auth_token::Entity::find()
            .filter(Expr::col((AuthTokenIden::Table, AuthTokenIden::Token)).eq(token))
            .find_also_related(user::Entity)
            .one(&self.db_con)
            .await?
            .ok_or(DbError::TokenNotFound)?;
        let u = u.ok_or(DbError::TokenNotFound)?;

        // Cargo sends the token with every request, so the date is only
        // written when it is older than the interval. A failed update must not
        // fail the request, the date is informational.
        let now = Utc::now();
        let stale_before = (now - TOKEN_LAST_USED_INTERVAL)
            .format(DB_DATE_FORMAT)
            .to_string();
        if at
            .last_used
            .as_ref()
            .is_none_or(|used| *used < stale_before)
        {
            let mut at: auth_token::ActiveModel = at.into();
            at.last_used = Set(Some(now.format(DB_DATE_FORMAT).to_string()));
            if let Err(e) = at.update(&self.db_con).await {
                warn!("Failed to update the last use of an auth token: {e}");
            }
        }

        Ok(User::from(u))
    }

    pub(super) async fn get_auth_token_name(&self, token: &PlainToken) -> DbResult<Option<String>> {
        Ok(auth_token::Entity::find()
            .filter(auth_token::Column::Token.eq(hash_token(token.expose_secret())))
            .one(&self.db_con)
            .await?
            .map(|at| at.name))
    }

    pub(super) async fn get_auth_tokens(&self, user_name: &UserName) -> DbResult<Vec<AuthToken>> {
        let at: Vec<auth_token::Model> = auth_token::Entity::find()
            .join(JoinType::InnerJoin, auth_token::Relation::User.def())
            .filter(user::Column::Name.eq(user_name))
            .all(&self.db_con)
            .await?;

        Ok(at.into_iter().map(AuthToken::from).collect())
    }

    pub(super) async fn delete_auth_token(
        &self,
        user_name: &UserName,
        id: AuthTokenId,
    ) -> DbResult<()> {
        let deleted = auth_token::Entity::delete_many()
            .filter(auth_token::Column::Id.eq(id))
            .filter(auth_token::Column::UserFk.in_subquery(Self::user_id_by_name(user_name)))
            .exec(&self.db_con)
            .await?;
        if deleted.rows_affected == 0 {
            return Err(DbError::TokenNotFound);
        }
        Ok(())
    }
}
