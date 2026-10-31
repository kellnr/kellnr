use chrono::Utc;
use kellnr_common::account_name::UserName;
use kellnr_entity::{oauth2_identity, oauth2_state, user};
use kellnr_settings::constants::OAUTH2_STATE_MAX_AGE;
use sea_orm::entity::prelude::Uuid;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, Set};

use super::{DB_DATE_FORMAT, Database};
use crate::error::DbError;
use crate::password::hash_pwd;
use crate::provider::{DbResult, OAuth2StateData};
use crate::{User, UserFlags, UserId};

/// Timestamp (in [`DB_DATE_FORMAT`]) before which an `OAuth2` state is expired.
fn oauth2_state_expiry_cutoff() -> String {
    let max_age =
        chrono::Duration::from_std(OAUTH2_STATE_MAX_AGE).expect("constant fits chrono::Duration");
    (Utc::now() - max_age).format(DB_DATE_FORMAT).to_string()
}

impl Database {
    pub(super) async fn get_user_by_oauth2_identity(
        &self,
        issuer: &str,
        subject: &str,
    ) -> DbResult<Option<User>> {
        let identity = oauth2_identity::Entity::find()
            .filter(oauth2_identity::Column::ProviderIssuer.eq(issuer))
            .filter(oauth2_identity::Column::Subject.eq(subject))
            .one(&self.db_con)
            .await?;

        if let Some(identity) = identity {
            let u = user::Entity::find_by_id(identity.user_fk)
                .one(&self.db_con)
                .await?
                .ok_or_else(|| DbError::UserNotFound(format!("user_id={}", identity.user_fk)))?;

            Ok(Some(User::from(u)))
        } else {
            Ok(None)
        }
    }

    pub(super) async fn has_oauth2_admin(&self) -> DbResult<bool> {
        let found = user::Entity::find()
            .filter(user::Column::IsAdmin.eq(true))
            .inner_join(oauth2_identity::Entity)
            .one(&self.db_con)
            .await?;

        Ok(found.is_some())
    }

    pub(super) async fn create_oauth2_user(
        &self,
        username: &UserName,
        issuer: &str,
        subject: &str,
        email: Option<String>,
        flags: UserFlags,
    ) -> DbResult<User> {
        // OAuth2 users do not use password auth, but the column needs a
        // valid hash. Hash before opening the transaction to keep it short.
        let random_pwd = Uuid::new_v4().to_string();
        let hashed_pwd = hash_pwd(&random_pwd).await;
        let created = Utc::now().format(DB_DATE_FORMAT).to_string();

        // Use a transaction to ensure atomicity
        let user_id = self
            .in_transaction(async |txn| {
                // Create the user
                let new_user = user::ActiveModel {
                    name: Set(username.to_string()),
                    pwd: Set(hashed_pwd),
                    salt: Set(String::new()),
                    is_admin: Set(flags.admin),
                    is_read_only: Set(flags.read_only),
                    created: Set(created.clone()),
                    ..Default::default()
                };

                let res = user::Entity::insert(new_user).exec(txn).await?;
                let user_id = res.last_insert_id;

                // Link the OAuth2 identity
                let identity = oauth2_identity::ActiveModel {
                    user_fk: Set(user_id),
                    provider_issuer: Set(issuer.to_string()),
                    subject: Set(subject.to_string()),
                    email: Set(email),
                    created: Set(created.clone()),
                    ..Default::default()
                };

                oauth2_identity::Entity::insert(identity).exec(txn).await?;

                Ok(user_id)
            })
            .await?;

        Ok(User {
            id: UserId::new(user_id),
            name: username.clone(),
            pwd: String::new(),  // Don't expose password hash
            salt: String::new(), // Don't expose salt
            is_admin: flags.admin,
            is_read_only: flags.read_only,
            created,
        })
    }

    pub(super) async fn store_oauth2_state(
        &self,
        state: &str,
        pkce_verifier: &str,
        nonce: &str,
    ) -> DbResult<()> {
        let created = Utc::now().format(DB_DATE_FORMAT).to_string();
        let s = oauth2_state::ActiveModel {
            state: Set(state.to_string()),
            pkce_verifier: Set(pkce_verifier.to_string()),
            nonce: Set(nonce.to_string()),
            created: Set(created),
            ..Default::default()
        };

        oauth2_state::Entity::insert(s).exec(&self.db_con).await?;
        Ok(())
    }

    pub(super) async fn get_and_delete_oauth2_state(
        &self,
        state: &str,
    ) -> DbResult<Option<OAuth2StateData>> {
        let Some(s) = oauth2_state::Entity::find()
            .filter(oauth2_state::Column::State.eq(state))
            .one(&self.db_con)
            .await?
        else {
            return Ok(None);
        };

        // The state is single use. Only the request whose delete actually
        // removed the row may continue, so two concurrent callbacks with the
        // same state cannot both succeed.
        let deleted = oauth2_state::Entity::delete_many()
            .filter(oauth2_state::Column::Id.eq(s.id))
            .exec(&self.db_con)
            .await?
            .rows_affected;
        if deleted == 0 || s.created < oauth2_state_expiry_cutoff() {
            return Ok(None);
        }

        Ok(Some(OAuth2StateData {
            state: s.state,
            pkce_verifier: s.pkce_verifier,
            nonce: s.nonce,
        }))
    }

    pub(super) async fn cleanup_expired_oauth2_states(&self) -> DbResult<u64> {
        let result = oauth2_state::Entity::delete_many()
            .filter(oauth2_state::Column::Created.lt(oauth2_state_expiry_cutoff()))
            .exec(&self.db_con)
            .await?;

        Ok(result.rows_affected)
    }
}
