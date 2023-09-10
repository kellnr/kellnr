use common::util::generate_rand_string;
use hex::ToHex;
use sha2::{Digest, Sha256};

const SALT_LENGTH: usize = 10;

pub fn hash_pwd(pwd: &str, salt: &str) -> String {
    let concat = format!("{}{}", pwd, salt);
    let result = Sha256::digest(concat.as_bytes());
    (&result[..]).encode_hex::<String>()
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
}
