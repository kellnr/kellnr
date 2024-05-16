use common::util::generate_rand_string;

const SALT_LENGTH: usize = 10;

pub fn hash_pwd(pwd: &str, salt: &str) -> String {
    let concat = format!("{}{}", pwd, salt);
    sha256::digest(concat)
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
