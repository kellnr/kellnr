use chrono::Utc;
use kellnr_common::account_name::UserName;
use kellnr_entity::user;
use sea_orm::query::QueryOrder;
use sea_orm::sea_query::{Expr, Func};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, ExprTrait, ModelTrait, QueryFilter, Set,
};
use tracing::warn;

use super::{DB_DATE_FORMAT, Database};
use crate::error::DbError;
use crate::password::{PwdVerification, hash_pwd, verify_pwd};
use crate::provider::DbResult;
use crate::{User, UserFlags};

impl Database {
    /// Replaces the legacy SHA-256 hash of `user` with an argon2 hash of `pwd`.
    /// The update only applies while the stored hash is still the legacy one,
    /// so a password changed concurrently is never overwritten.
    async fn upgrade_legacy_pwd_hash(&self, user: &User, pwd: &str) -> DbResult<()> {
        let hashed = hash_pwd(pwd).await;
        user::Entity::update_many()
            .col_expr(user::Column::Pwd, Expr::value(hashed))
            .col_expr(user::Column::Salt, Expr::value(String::new()))
            .filter(user::Column::Id.eq(user.id))
            .filter(user::Column::Pwd.eq(user.pwd.as_str()))
            .exec(&self.db_con)
            .await?;
        Ok(())
    }

    pub(super) async fn authenticate_user(&self, name: &UserName, pwd: &str) -> DbResult<User> {
        let user = self.get_user(name).await?;

        match verify_pwd(pwd, &user.pwd, &user.salt).await {
            PwdVerification::Valid => Ok(user),
            PwdVerification::ValidLegacy => {
                // Upgrade the legacy SHA-256 hash now that the plaintext is
                // known. A failure must not block an otherwise valid login,
                // the upgrade is retried on the next one.
                if let Err(e) = self.upgrade_legacy_pwd_hash(&user, pwd).await {
                    warn!("Failed to upgrade password hash of user {}: {e}", user.name);
                }
                Ok(user)
            }
            PwdVerification::Invalid => Err(DbError::PasswordMismatch),
        }
    }

    pub(super) async fn delete_user(&self, user_name: &UserName) -> DbResult<()> {
        self.get_user_model(user_name)
            .await?
            .delete(&self.db_con)
            .await?;
        Ok(())
    }

    pub(super) async fn change_pwd(&self, user_name: &UserName, new_pwd: &str) -> DbResult<()> {
        let hashed = hash_pwd(new_pwd).await;

        let mut u: user::ActiveModel = self.get_user_model(user_name).await?.into();
        u.pwd = Set(hashed);
        // The argon2 PHC string carries its own salt.
        u.salt = Set(String::new());

        u.update(&self.db_con).await?;
        Ok(())
    }

    pub(super) async fn change_read_only_state(
        &self,
        user_name: &UserName,
        state: bool,
    ) -> DbResult<()> {
        let mut u: user::ActiveModel = self.get_user_model(user_name).await?.into();
        u.is_read_only = Set(state);

        u.update(&self.db_con).await?;
        Ok(())
    }

    pub(super) async fn change_admin_state(
        &self,
        user_name: &UserName,
        state: bool,
    ) -> DbResult<()> {
        let mut u: user::ActiveModel = self.get_user_model(user_name).await?.into();
        u.is_admin = Set(state);

        u.update(&self.db_con).await?;
        Ok(())
    }

    pub(super) async fn get_user(&self, name: &UserName) -> DbResult<User> {
        Ok(User::from(self.get_user_model(name).await?))
    }

    pub(super) async fn add_user(
        &self,
        name: &UserName,
        pwd: &str,
        flags: UserFlags,
    ) -> DbResult<()> {
        let hashed_pwd = hash_pwd(pwd).await;
        let created = Utc::now().format(DB_DATE_FORMAT).to_string();

        let u = user::ActiveModel {
            name: Set(name.as_str().to_owned()),
            pwd: Set(hashed_pwd),
            salt: Set(String::new()),
            is_admin: Set(flags.admin),
            is_read_only: Set(flags.read_only),
            created: Set(created),
            ..Default::default()
        };

        u.insert(&self.db_con).await?;
        Ok(())
    }

    pub(super) async fn get_users(&self) -> DbResult<Vec<User>> {
        let users = user::Entity::find()
            .order_by_asc(user::Column::Name)
            .all(&self.db_con)
            .await?;

        Ok(users.into_iter().map(User::from).collect())
    }

    pub(super) async fn is_username_available(&self, username: &UserName) -> DbResult<bool> {
        // Names that only differ in case are too easy to confuse.
        let existing = user::Entity::find()
            .filter(Func::lower(Expr::col(user::Column::Name)).eq(username.as_str().to_lowercase()))
            .one(&self.db_con)
            .await?;

        Ok(existing.is_none())
    }
}
