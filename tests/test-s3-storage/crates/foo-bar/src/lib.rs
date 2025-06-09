//! # test_lib
//! `test_lib` is a test library for Kellnr.
//! It is used to test upload, download and Rustdoc functionality.


use regex::Regex;

/// Test function that returns a string.
pub fn test_fn() -> String {
    let _ = Regex::new(r"^[a-zA-Z][a-zA-Z0-9-_]*$").unwrap();
    "Hello from test".to_string()
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
