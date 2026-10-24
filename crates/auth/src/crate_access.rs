use std::sync::Arc;

use kellnr_common::normalized_name::NormalizedName;
use kellnr_db::provider::DbResult;
use kellnr_db::{CrateVisibility, DbProvider};

use crate::maybe_user::MaybeUser;

/// Whether `user` may read the content of a download-restricted crate,
/// i.e. the crate archive and its hosted rustdoc.
pub async fn has_restricted_crate_access(
    crate_name: &NormalizedName,
    user: &str,
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
    crate_name: &NormalizedName,
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

    fn name() -> NormalizedName {
        NormalizedName::from_unchecked("restricted".to_string())
    }

    fn user(is_admin: bool) -> MaybeUser {
        MaybeUser {
            name: "user".to_string(),
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
            CrateVisibility::User("user".to_string()),
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
            .with(eq(name()), eq("user"))
            .returning(|_, _| Ok(true));
        let db: Arc<dyn DbProvider> = Arc::new(db);
        assert!(
            can_read_crate(&name(), Some(&user(false)), &db)
                .await
                .unwrap()
        );
    }
}
