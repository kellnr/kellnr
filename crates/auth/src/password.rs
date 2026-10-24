use kellnr_common::login_throttle::LoginThrottle;
use kellnr_db::{DbProvider, User};

#[derive(Debug, PartialEq, Eq)]
pub enum PasswordAuthError {
    /// Too many attempts for this username; the password was not checked.
    Throttled,
    /// Unknown user or wrong password.
    Invalid,
}

/// Checks a username and password, subject to the per-username attempt
/// throttle. Every password check in Kellnr goes through here, so brute
/// forcing is limited no matter which endpoint is used.
pub async fn authenticate_password(
    db: &dyn DbProvider,
    throttle: &LoginThrottle,
    user: &str,
    pwd: &str,
) -> Result<User, PasswordAuthError> {
    if !throttle.try_attempt(user).await {
        return Err(PasswordAuthError::Throttled);
    }

    let user = db
        .authenticate_user(user, pwd)
        .await
        .map_err(|_| PasswordAuthError::Invalid)?;
    throttle.succeeded(&user.name).await;
    Ok(user)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use kellnr_db::error::DbError;
    use kellnr_db::mock::MockDb;

    use super::*;

    fn throttle(max_attempts: u32) -> LoginThrottle {
        LoginThrottle::new(max_attempts, Duration::from_mins(5))
    }

    fn user() -> User {
        User {
            name: "user".to_string(),
            ..User::default()
        }
    }

    #[tokio::test]
    async fn valid_password_authenticates() {
        let mut db = MockDb::new();
        db.expect_authenticate_user().returning(|_, _| Ok(user()));

        let result = authenticate_password(&db, &throttle(3), "user", "pwd").await;

        assert_eq!(result.unwrap().name, "user");
    }

    /// Once the limit is reached, the password is not checked at all, so a
    /// correct guess cannot be confirmed either.
    #[tokio::test]
    async fn throttles_after_limit_without_checking_password() {
        let mut db = MockDb::new();
        db.expect_authenticate_user()
            .times(2)
            .returning(|_, _| Err(DbError::PasswordMismatch));
        let throttle = throttle(2);

        for _ in 0..2 {
            let result = authenticate_password(&db, &throttle, "user", "wrong").await;
            assert_eq!(result.unwrap_err(), PasswordAuthError::Invalid);
        }
        let result = authenticate_password(&db, &throttle, "user", "right").await;
        assert_eq!(result.unwrap_err(), PasswordAuthError::Throttled);
    }

    #[tokio::test]
    async fn unknown_user_is_throttled() {
        let mut db = MockDb::new();
        db.expect_authenticate_user()
            .times(1)
            .returning(|name, _| Err(DbError::UserNotFound(name.to_string())));
        let throttle = throttle(1);

        let first = authenticate_password(&db, &throttle, "ghost", "pwd").await;
        let second = authenticate_password(&db, &throttle, "ghost", "pwd").await;

        assert_eq!(first.unwrap_err(), PasswordAuthError::Invalid);
        assert_eq!(second.unwrap_err(), PasswordAuthError::Throttled);
    }

    #[tokio::test]
    async fn success_resets_attempts() {
        let mut db = MockDb::new();
        db.expect_authenticate_user()
            .withf(|_, pwd| pwd == "right")
            .returning(|_, _| Ok(user()));
        db.expect_authenticate_user()
            .withf(|_, pwd| pwd == "wrong")
            .returning(|_, _| Err(DbError::PasswordMismatch));
        let throttle = throttle(2);

        let _ = authenticate_password(&db, &throttle, "user", "wrong").await;
        authenticate_password(&db, &throttle, "user", "right")
            .await
            .unwrap();

        // Both attempts are available again after the successful login.
        for _ in 0..2 {
            let result = authenticate_password(&db, &throttle, "user", "wrong").await;
            assert_eq!(result.unwrap_err(), PasswordAuthError::Invalid);
        }
    }
}
