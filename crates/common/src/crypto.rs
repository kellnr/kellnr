use rand::{Rng, distr::Alphanumeric, rng};
use std::iter;

mod crypto_new {
    use alkali::hash::pbkdf;

    #[derive(Debug, Eq, PartialEq, thiserror::Error)]
    pub enum CryptoError {
        #[error("Incorrect password")]
        PasswordIncorrect,
        #[error("Failed to hash password: {0}")]
        FailedToHashPassword(String),
        #[error("Failed to verify password: {0}")]
        FailedToVerifyPassword(String),
    }

    /// Hash a `password` for storage and later identity verification with [`verify_password`].
    pub fn store_password(password: &str) -> Result<String, CryptoError> {
        pbkdf::store_password(
            password,
            constants::PBKDF_AUTH_OPS_LIMIT,
            constants::PBKDF_AUTH_MEM_LIMIT,
        )
        .map_err(|err| CryptoError::FailedToHashPassword(err.to_string()))
    }

    /// Verify `password` matches the provided `hash`, returned by [`store_password`].
    pub fn verify_password(password: &str, hash: &str) -> Result<(), CryptoError> {
        pbkdf::verify_password(password, hash).map_err(|err| match err {
            alkali::AlkaliError::PasswordHashError(pbkdf::PasswordHashError::PasswordIncorrect) => {
                CryptoError::PasswordIncorrect
            }
            other => CryptoError::FailedToVerifyPassword(other.to_string()),
        })
    }

    mod constants {
        use alkali::hash::pbkdf;
        pub const PBKDF_AUTH_OPS_LIMIT: usize = pbkdf::OPS_LIMIT_INTERACTIVE;
        pub const PBKDF_AUTH_MEM_LIMIT: usize = pbkdf::MEM_LIMIT_INTERACTIVE;
    }
}

pub fn generate_rand_string(length: usize) -> String {
    let mut rng = rng();
    iter::repeat(())
        .map(|()| rng.sample(Alphanumeric))
        .map(char::from)
        .take(length)
        .collect::<String>()
}

pub fn generate_token() -> String {
    let mut rng = rng();
    iter::repeat(())
        .map(|()| rng.sample(Alphanumeric))
        .map(char::from)
        .take(32)
        .collect::<String>()
}

const SALT_LENGTH: usize = 10;

pub fn hash_pwd(pwd: &str, salt: &str) -> String {
    let concat = format!("{pwd}{salt}");
    sha256::digest(concat)
}

pub fn hash_token(token: &str) -> String {
    sha256::digest(token)
}

pub fn hash_file_sha256(data: &[u8]) -> String {
    sha256::digest(data)
}

pub fn generate_salt() -> String {
    generate_rand_string(SALT_LENGTH)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_salt_creates_random_string() {
        let s1 = generate_salt();
        let s2 = generate_salt();

        assert_eq!(SALT_LENGTH, s1.len());
        assert_eq!(SALT_LENGTH, s2.len());
        assert_ne!(s1, s2);
    }

    #[test]
    fn hash_pwd_computes_correct_hash() {
        let pwd = "admin";
        let salt = "C6udtgbngX";
        let hash = hash_pwd(pwd, salt);

        assert_eq!(
            hash,
            "5dcec54caf0f55652766f71c32a0eac6538e7faeeab9301f956a58b7dbad02fb"
        );
    }
}
