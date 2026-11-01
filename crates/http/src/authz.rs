use std::sync::Arc;

use axum::http::StatusCode;
use kellnr_common::account_name::UserName;
use kellnr_common::canonical_name::CanonicalName;
use kellnr_db::provider::DbResult;
use kellnr_db::{CrateVisibility, DbProvider};

use crate::error::ApiError;
use crate::identity::Identity;

/// Fail unless `user` is an admin.
pub fn require_admin(user: &Identity) -> Result<(), ApiError> {
    if user.is_admin {
        Ok(())
    } else {
        Err(ApiError::new(
            "Admin access required",
            StatusCode::FORBIDDEN,
        ))
    }
}

/// Fail unless `user` owns the crate or is an admin.
pub async fn require_owner(
    crate_name: &CanonicalName,
    user: &Identity,
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

/// Fail if `user` is read-only. Admins can always modify, as they can change
/// their read-only state anyway.
pub fn require_writable(user: &Identity) -> Result<(), ApiError> {
    if !user.is_admin && user.is_read_only {
        Err(ApiError::new(
            "Read-only users cannot modify the registry",
            StatusCode::FORBIDDEN,
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
    user: Option<&Identity>,
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
pub fn crate_visibility(user: Option<&Identity>) -> CrateVisibility {
    match user {
        None => CrateVisibility::Unrestricted,
        Some(user) if user.is_admin => CrateVisibility::All,
        Some(user) => CrateVisibility::User(user.name.clone()),
    }
}

#[cfg(test)]
mod tests {
    use kellnr_common::account_name::GroupName;
    use kellnr_db::UserFlags;
    use kellnr_db::test_utils::TestCrateBuilder;
    use kellnr_test_db::TestDb;

    use super::*;
    use crate::identity::Source;

    fn name() -> CanonicalName {
        CanonicalName::try_from("restricted").unwrap()
    }

    fn user_name() -> UserName {
        UserName::new("user").unwrap()
    }

    fn user(is_admin: bool) -> Identity {
        Identity {
            name: user_name(),
            is_admin,
            is_read_only: false,
            source: Source::Session,
        }
    }

    /// A database with the crate `restricted`, owned by `admin`, and the
    /// user `user` without any access to it.
    async fn crate_db(restricted: bool) -> TestDb {
        let db = TestDb::sqlite().await;
        TestCrateBuilder::new(&db)
            .name("restricted")
            .build()
            .await
            .unwrap();
        db.change_download_restricted(&name(), restricted)
            .await
            .unwrap();
        db.add_user(&user_name(), "pwd", UserFlags::default())
            .await
            .unwrap();
        db
    }

    async fn can_read(db: &TestDb, user: Option<&Identity>) -> bool {
        can_read_crate(&name(), user, &db.provider()).await.unwrap()
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
        let db = crate_db(false).await;
        assert!(can_read(&db, None).await);
    }

    #[tokio::test]
    async fn restricted_crate_is_hidden_from_anonymous() {
        let db = crate_db(true).await;
        assert!(!can_read(&db, None).await);
    }

    #[tokio::test]
    async fn restricted_crate_is_readable_by_admin() {
        let db = crate_db(true).await;
        assert!(can_read(&db, Some(&user(true))).await);
    }

    #[tokio::test]
    async fn restricted_crate_is_hidden_from_user_without_access() {
        let db = crate_db(true).await;
        assert!(!can_read(&db, Some(&user(false))).await);
    }

    #[tokio::test]
    async fn restricted_crate_is_readable_by_crate_user() {
        let db = crate_db(true).await;
        db.add_crate_user(&name(), &user_name()).await.unwrap();
        assert!(can_read(&db, Some(&user(false))).await);
    }

    #[tokio::test]
    async fn restricted_crate_is_readable_by_crate_group_member() {
        let db = crate_db(true).await;
        let group = GroupName::new("group").unwrap();
        db.add_group(&group).await.unwrap();
        db.add_group_user(&group, &user_name()).await.unwrap();
        db.add_crate_group(&name(), &group).await.unwrap();
        assert!(can_read(&db, Some(&user(false))).await);
    }

    #[tokio::test]
    async fn restricted_crate_is_readable_by_owner() {
        let db = crate_db(true).await;
        db.add_owner(&name(), &user_name()).await.unwrap();
        assert!(can_read(&db, Some(&user(false))).await);
    }
}

#[cfg(test)]
mod require_tests {
    use super::*;
    use crate::identity::Source;

    fn user(is_admin: bool, is_read_only: bool) -> Identity {
        Identity {
            name: UserName::new("user").unwrap(),
            is_admin,
            is_read_only,
            source: Source::Token(None),
        }
    }

    fn status(result: Result<(), ApiError>) -> Option<StatusCode> {
        result.err().map(|e| e.status())
    }

    #[test]
    fn only_admins_pass_require_admin() {
        assert_eq!(status(require_admin(&user(true, false))), None);
        assert_eq!(
            status(require_admin(&user(false, false))),
            Some(StatusCode::FORBIDDEN)
        );
    }

    #[test]
    fn read_only_users_are_forbidden_to_write() {
        assert_eq!(
            status(require_writable(&user(false, true))),
            Some(StatusCode::FORBIDDEN)
        );
        assert_eq!(status(require_writable(&user(false, false))), None);
        assert_eq!(status(require_writable(&user(true, true))), None);
    }
}
