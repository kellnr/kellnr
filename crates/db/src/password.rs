use argon2::{Algorithm, Argon2, Params, Version};
use password_hash::rand_core::OsRng;
use password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use subtle::ConstantTimeEq;

/// Outcome of checking a password against its stored hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PwdVerification {
    Valid,
    /// The password matches a legacy SHA-256 hash and should be rehashed.
    ValidLegacy,
    Invalid,
}

fn argon2() -> Argon2<'static> {
    // argon2id with the OWASP recommended minimum (19 MiB, 2 iterations).
    Argon2::new(Algorithm::Argon2id, Version::V0x13, Params::default())
}

/// Hashes a password with argon2id and returns it in PHC string format. The
/// random salt is part of the returned string.
///
/// Hashing is CPU heavy by design, so it runs on the blocking thread pool to
/// keep the async workers responsive.
pub async fn hash_pwd(pwd: &str) -> String {
    let pwd = pwd.to_owned();
    run_blocking(move || hash_pwd_sync(&pwd)).await
}

/// Verifies `pwd` against `stored_hash`. Hashes in PHC format are checked with
/// argon2. Anything else is treated as a legacy `sha256(pwd + salt)` hash
/// created by older Kellnr versions, which uses the separately stored salt.
pub async fn verify_pwd(pwd: &str, stored_hash: &str, legacy_salt: &str) -> PwdVerification {
    let (pwd, stored_hash, legacy_salt) = (
        pwd.to_owned(),
        stored_hash.to_owned(),
        legacy_salt.to_owned(),
    );
    run_blocking(move || verify_pwd_sync(&pwd, &stored_hash, &legacy_salt)).await
}

async fn run_blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    tokio::task::spawn_blocking(f)
        .await
        .expect("password hashing task panicked")
}

fn hash_pwd_sync(pwd: &str) -> String {
    let salt = SaltString::generate(&mut OsRng);
    argon2()
        .hash_password(pwd.as_bytes(), &salt)
        .expect("argon2 hashing with default parameters cannot fail")
        .to_string()
}

fn verify_pwd_sync(pwd: &str, stored_hash: &str, legacy_salt: &str) -> PwdVerification {
    if stored_hash.starts_with('$') {
        return match PasswordHash::new(stored_hash) {
            Ok(parsed) if argon2().verify_password(pwd.as_bytes(), &parsed).is_ok() => {
                PwdVerification::Valid
            }
            _ => PwdVerification::Invalid,
        };
    }

    let legacy = legacy_hash(pwd, legacy_salt);
    if bool::from(legacy.as_bytes().ct_eq(stored_hash.as_bytes())) {
        PwdVerification::ValidLegacy
    } else {
        PwdVerification::Invalid
    }
}

pub(crate) fn legacy_hash(pwd: &str, salt: &str) -> String {
    sha256::digest(format!("{pwd}{salt}"))
}

pub fn hash_token(token: &str) -> String {
    sha256::digest(token)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_pwd_produces_argon2id_phc_string() {
        let hash = hash_pwd_sync("admin");

        assert!(
            hash.starts_with("$argon2id$v=19$"),
            "unexpected hash {hash}"
        );
    }

    #[test]
    fn hash_pwd_uses_random_salt() {
        assert_ne!(hash_pwd_sync("admin"), hash_pwd_sync("admin"));
    }

    #[test]
    fn verify_pwd_accepts_argon2_hash() {
        let hash = hash_pwd_sync("admin");

        assert_eq!(verify_pwd_sync("admin", &hash, ""), PwdVerification::Valid);
        assert_eq!(
            verify_pwd_sync("wrong", &hash, ""),
            PwdVerification::Invalid
        );
    }

    #[test]
    fn verify_pwd_accepts_legacy_hash() {
        let salt = "C6udtgbngX";
        let legacy = "5dcec54caf0f55652766f71c32a0eac6538e7faeeab9301f956a58b7dbad02fb";

        assert_eq!(
            verify_pwd_sync("admin", legacy, salt),
            PwdVerification::ValidLegacy
        );
        assert_eq!(
            verify_pwd_sync("wrong", legacy, salt),
            PwdVerification::Invalid
        );
        assert_eq!(
            verify_pwd_sync("admin", legacy, "other_salt"),
            PwdVerification::Invalid
        );
    }

    #[test]
    fn verify_pwd_rejects_malformed_phc_string() {
        assert_eq!(
            verify_pwd_sync("admin", "$argon2id$garbage", ""),
            PwdVerification::Invalid
        );
    }

    #[test]
    fn verify_pwd_rejects_empty_hash() {
        assert_eq!(verify_pwd_sync("", "", ""), PwdVerification::Invalid);
    }

    #[test]
    fn legacy_hash_computes_sha256_of_pwd_and_salt() {
        assert_eq!(
            legacy_hash("admin", "C6udtgbngX"),
            "5dcec54caf0f55652766f71c32a0eac6538e7faeeab9301f956a58b7dbad02fb"
        );
    }
}
