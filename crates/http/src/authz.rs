use std::sync::Arc;

use axum::http::StatusCode;
use kellnr_common::account_name::UserName;
use kellnr_common::canonical_name::CanonicalName;
use kellnr_db::provider::DbResult;
use kellnr_db::{CrateVisibility, DbProvider};

use crate::error::ApiError;
use crate::maybe_user::MaybeUser;

/// Fail unless `user` owns the crate or is an admin.
pub async fn check_ownership(
    crate_name: &CanonicalName,
    user: &MaybeUser,
    db: &Arc<dyn DbProvider>,
) -> Result<(), ApiError> {
    if user.is_admin || db.is_owner(crate_name, &user.name).await? {
        Ok(())
    } else {
        Err(ApiError::new(
            "Not the owner of the crate",
            StatusCode::FORBIDDEN,
        ))
    }
}

/// Fail if `user` is read-only. Admins can always modify.
pub fn check_can_modify(user: &MaybeUser) -> Result<(), ApiError> {
    if !user.is_admin && user.is_read_only {
        Err(ApiError::new(
            "Read-only users cannot modify the registry",
            StatusCode::BAD_REQUEST,
        ))
    } else {
        Ok(())
    }
}

/// Whether `user` may read the content of a download-restricted crate,
/// i.e. the crate archive and its hosted rustdoc.
pub async fn has_restricted_crate_access(
    crate_name: &CanonicalName,
    user: &UserName,
    is_admin: bool,
    db: &Arc<dyn DbProvider>,
) -> DbResult<bool> {
    Ok(is_admin
        || db.is_crate_user(crate_name, user).await?
        || db.is_crate_group_user(crate_name, user).await?
        || db.is_owner(crate_name, user).await?)
}

/// Whether `user` may see a crate and its metadata, honoring its download
/// restriction. `None` is an anonymous caller, who only sees unrestricted crates.
pub async fn can_read_crate(
    crate_name: &CanonicalName,
    user: Option<&MaybeUser>,
    db: &Arc<dyn DbProvider>,
) -> DbResult<bool> {
    if !db.is_download_restricted(crate_name).await? {
        return Ok(true);
    }
    match user {
        Some(user) => has_restricted_crate_access(crate_name, &user.name, user.is_admin, db).await,
        None => Ok(false),
    }
}

/// Which download-restricted crates `user` may see in crate listings.
pub fn crate_visibility(user: Option<&MaybeUser>) -> CrateVisibility {
    match user {
        None => CrateVisibility::Unrestricted,
        Some(user) if user.is_admin => CrateVisibility::All,
        Some(user) => CrateVisibility::User(user.name.clone()),
    }
}

#[cfg(test)]
mod tests {
    use kellnr_db::mock::MockDb;
    use mockall::predicate::eq;

    use super::*;

    fn name() -> CanonicalName {
        CanonicalName::try_from("restricted").unwrap()
    }

    fn user(is_admin: bool) -> MaybeUser {
        MaybeUser {
            name: UserName::new("user").unwrap(),
            is_admin,
            is_read_only: false,
        }
    }

    fn restricted_db(restricted: bool) -> MockDb {
        let mut db = MockDb::new();
        db.expect_is_download_restricted()
            .with(eq(name()))
            .returning(move |_| Ok(restricted));
        db
    }

    #[test]
    fn crate_visibility_depends_on_caller() {
        assert_eq!(CrateVisibility::Unrestricted, crate_visibility(None));
        assert_eq!(CrateVisibility::All, crate_visibility(Some(&user(true))));
        assert_eq!(
            CrateVisibility::User(UserName::new("user").unwrap()),
            crate_visibility(Some(&user(false)))
        );
    }

    #[tokio::test]
    async fn unrestricted_crate_is_readable_by_anyone() {
        let db: Arc<dyn DbProvider> = Arc::new(restricted_db(false));
        assert!(can_read_crate(&name(), None, &db).await.unwrap());
    }

    #[tokio::test]
    async fn restricted_crate_is_hidden_from_anonymous() {
        let db: Arc<dyn DbProvider> = Arc::new(restricted_db(true));
        assert!(!can_read_crate(&name(), None, &db).await.unwrap());
    }

    #[tokio::test]
    async fn restricted_crate_is_readable_by_admin() {
        let db: Arc<dyn DbProvider> = Arc::new(restricted_db(true));
        assert!(
            can_read_crate(&name(), Some(&user(true)), &db)
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn restricted_crate_is_hidden_from_user_without_access() {
        let mut db = restricted_db(true);
        db.expect_is_crate_user().returning(|_, _| Ok(false));
        db.expect_is_crate_group_user().returning(|_, _| Ok(false));
        db.expect_is_owner().returning(|_, _| Ok(false));
        let db: Arc<dyn DbProvider> = Arc::new(db);
        assert!(
            !can_read_crate(&name(), Some(&user(false)), &db)
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn restricted_crate_is_readable_by_crate_user() {
        let mut db = restricted_db(true);
        db.expect_is_crate_user()
            .with(eq(name()), eq(UserName::new("user").unwrap()))
            .returning(|_, _| Ok(true));
        let db: Arc<dyn DbProvider> = Arc::new(db);
        assert!(
            can_read_crate(&name(), Some(&user(false)), &db)
                .await
                .unwrap()
        );
    }
}
