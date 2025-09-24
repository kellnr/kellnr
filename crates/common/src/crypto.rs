pub use new::generate_rand_string;
pub use new::generate_token;
pub use new::store_password;
pub use new::store_token;
pub use new::verify_password;
pub use new::verify_token;

pub mod update {
    pub use crate::crypto::new::CryptoError;
    use crate::crypto::new::generate_rand_string;
    use crate::crypto::new::verify_password;
    use crate::crypto::new::verify_token;

    /// Signals if the password hash should be updated to a new hash
    pub enum ShouldMigrateHash {
        // No
        Keep,
        //Yes
        Update,
    }

    /// Verifies the password againts the given hash.
    /// Gives advise if the saved hash should be updated to a new hash,
    /// for example due to a change in the hashing algorithm
    pub fn verify_password_with_advise(
        password: &str,
        salt: &str,
        hash: &str,
    ) -> Result<ShouldMigrateHash, CryptoError> {
        if hash.starts_with("$argon2id") {
            verify_password(password, hash)?;
            // function would have returned if password does not matched given hash
            Ok(ShouldMigrateHash::Keep)
        } else if crate::crypto::old::hash_pwd(password, salt) == hash {
            Ok(ShouldMigrateHash::Update)
        } else {
            Err(CryptoError::PasswordIncorrect)
        }
    }

    /// Hashes the token.
    /// Gives advise if the token should be updated to a new hash,
    /// for example due to a change in the hashing algorithm
    pub fn store_token_with_advise(
        token: &str,
        hash: &str,
    ) -> Result<ShouldMigrateHash, CryptoError> {
        if hash.starts_with("$argon2id") {
            verify_token(token, hash)?;
            // function would have returned if password does not matched given hash
            Ok(ShouldMigrateHash::Keep)
        } else if crate::crypto::old::hash_token(token) == hash {
            Ok(ShouldMigrateHash::Update)
        } else {
            Err(CryptoError::PasswordIncorrect)
        }
    }

    pub fn generate_salt() -> String {
        // set salt to something random as fallback
        let rndm = generate_rand_string(32);
        format!("MIGRATEDTOARGON2ID{rndm}")
    }
}

mod new {
    use alkali::hash::pbkdf;
    use rand::Rng;
    use rand::distributions::Alphanumeric;

    #[derive(Debug, Eq, PartialEq, thiserror::Error)]
    pub enum CryptoError {
        #[error("Incorrect password")]
        PasswordIncorrect,
        #[error("Failed to hash password: {0}")]
        FailedToHashPassword(String),
        #[error("Failed to hash token: {0}")]
        FailedToHashToken(String),
        #[error("Failed to verify password: {0}")]
        FailedToVerifyPassword(String),
        #[error("Failed to verify token: {0}")]
        FailedToVerifyToken(String),
        #[error("Failed to generate random string: {0}")]
        FailedToGenerateRandomString(String),
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

    pub fn generate_rand_string(length: usize) -> String {
        let mut rng = alkali::random::SodiumRng;
        std::iter::repeat(())
            .map(|()| rng.sample(Alphanumeric))
            .map(char::from)
            .take(length)
            .collect::<String>()
    }

    pub fn store_token(token: &str) -> Result<String, CryptoError> {
        let digest = alkali::hash::generic::hash(token.as_bytes(), None)
            .map_err(|err| CryptoError::FailedToHashToken(err.to_string()))?;
        alkali::encode::hex::encode(&digest)
            .map_err(|err| CryptoError::FailedToHashToken(err.to_string()))
    }

    pub fn verify_token(token: &str, hash: &str) -> Result<(), CryptoError> {
        if alkali::mem::eq(token.as_bytes(), hash.as_bytes())
            .map_err(|err| CryptoError::FailedToVerifyPassword(err.to_string()))
            .is_ok_and(|x| x)
        {
            Ok(())
        } else {
            Err(CryptoError::PasswordIncorrect)
        }
    }

    pub fn generate_token() -> String {
        generate_rand_string(64)
    }

    mod constants {
        use alkali::hash::pbkdf;
        pub const PBKDF_AUTH_OPS_LIMIT: usize = pbkdf::OPS_LIMIT_INTERACTIVE;
        pub const PBKDF_AUTH_MEM_LIMIT: usize = pbkdf::MEM_LIMIT_INTERACTIVE;
    }
}

pub fn hash_file_sha256(data: &[u8]) -> String {
    sha256::digest(data)
}

mod old {
    pub fn hash_pwd(pwd: &str, salt: &str) -> String {
        let concat = format!("{pwd}{salt}");
        sha256::digest(concat)
    }

    pub fn hash_token(token: &str) -> String {
        sha256::digest(token)
    }
}
