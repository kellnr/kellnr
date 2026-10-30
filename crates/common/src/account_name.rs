//! Validation of the names of users, groups and auth tokens.

use thiserror::Error;

/// Maximal length of user and group names in characters.
pub const MAX_NAME_LEN: usize = 64;

/// Maximal length of auth token names in characters.
pub const MAX_TOKEN_NAME_LEN: usize = 128;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AccountNameError {
    #[error("The name must not be empty")]
    Empty,
    #[error("The name must not be longer than {0} characters")]
    TooLong(usize),
    #[error("The name must not contain control characters")]
    ControlCharacter,
    #[error("The user name must not contain ':'")]
    Colon,
}

/// Checks a new user name. A `:` is rejected because HTTP Basic auth splits
/// the credentials at the first `:`, so such a user could not log in.
pub fn validate_user_name(name: &str) -> Result<(), AccountNameError> {
    validate(name, MAX_NAME_LEN)?;
    if name.contains(':') {
        return Err(AccountNameError::Colon);
    }
    Ok(())
}

/// Checks a new group name.
pub fn validate_group_name(name: &str) -> Result<(), AccountNameError> {
    validate(name, MAX_NAME_LEN)
}

/// Checks a new auth token name.
pub fn validate_token_name(name: &str) -> Result<(), AccountNameError> {
    validate(name, MAX_TOKEN_NAME_LEN)
}

fn validate(name: &str, max_len: usize) -> Result<(), AccountNameError> {
    if name.trim().is_empty() {
        return Err(AccountNameError::Empty);
    }
    if name.chars().count() > max_len {
        return Err(AccountNameError::TooLong(max_len));
    }
    if name.chars().any(char::is_control) {
        return Err(AccountNameError::ControlCharacter);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_names_are_accepted() {
        assert_eq!(validate_user_name("alice.smith"), Ok(()));
        assert_eq!(validate_user_name(&"a".repeat(MAX_NAME_LEN)), Ok(()));
        assert_eq!(validate_group_name("dev team"), Ok(()));
        assert_eq!(validate_token_name("CI token: main"), Ok(()));
    }

    #[test]
    fn empty_and_whitespace_names_are_rejected() {
        assert_eq!(validate_user_name(""), Err(AccountNameError::Empty));
        assert_eq!(validate_group_name("  "), Err(AccountNameError::Empty));
        assert_eq!(validate_token_name("\t"), Err(AccountNameError::Empty));
    }

    #[test]
    fn long_names_are_rejected() {
        assert_eq!(
            validate_user_name(&"a".repeat(MAX_NAME_LEN + 1)),
            Err(AccountNameError::TooLong(MAX_NAME_LEN))
        );
        assert_eq!(
            validate_token_name(&"a".repeat(MAX_TOKEN_NAME_LEN + 1)),
            Err(AccountNameError::TooLong(MAX_TOKEN_NAME_LEN))
        );
    }

    #[test]
    fn control_characters_are_rejected() {
        assert_eq!(
            validate_user_name("ali\nce"),
            Err(AccountNameError::ControlCharacter)
        );
    }

    #[test]
    fn colon_is_rejected_in_user_names_only() {
        assert_eq!(validate_user_name("a:b"), Err(AccountNameError::Colon));
        assert_eq!(validate_group_name("a:b"), Ok(()));
    }
}
