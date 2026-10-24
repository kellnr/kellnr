//! Webhook signatures following the Standard Webhooks specification:
//! <https://github.com/standard-webhooks/standard-webhooks/blob/main/spec/standard-webhooks.md#verifying-webhook-authenticity>

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;

const SECRET_PREFIX: &str = "whsec_";
const MIN_SECRET_BYTES: usize = 24;

/// Decodes a webhook secret given as base64, optionally prefixed with
/// `whsec_`, the format used by the Standard Webhooks libraries.
pub(crate) fn decode_secret(secret: &str) -> Result<Vec<u8>, String> {
    let encoded = secret.strip_prefix(SECRET_PREFIX).unwrap_or(secret);
    let key = STANDARD
        .decode(encoded)
        .map_err(|_| "Webhook secret must be base64 encoded".to_string())?;
    if key.len() < MIN_SECRET_BYTES {
        return Err(format!(
            "Webhook secret must be at least {MIN_SECRET_BYTES} bytes long"
        ));
    }
    Ok(key)
}

/// Returns the `webhook-signature` header value for a delivery:
/// `v1,<base64 HMAC-SHA256 of "{msg_id}.{timestamp}.{body}">`.
pub(crate) fn sign(key: &[u8], msg_id: &str, timestamp: i64, body: &[u8]) -> String {
    let mut mac =
        Hmac::<Sha256>::new_from_slice(key).expect("HMAC-SHA256 accepts keys of any length");
    mac.update(format!("{msg_id}.{timestamp}.").as_bytes());
    mac.update(body);
    format!("v1,{}", STANDARD.encode(mac.finalize().into_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test vector of the Standard Webhooks reference implementations.
    const SECRET: &str = "whsec_MfKQ9r8GKYqrTwjUPD8ILPZIo2LaLaSw";
    const MSG_ID: &str = "msg_p5jXN8AQM9LWM0D4loKWxJek";
    const TIMESTAMP: i64 = 1_614_265_330;
    const BODY: &str = r#"{"test": 2432232314}"#;
    const SIGNATURE: &str = "v1,g0hM9SsE+OTPJTGt/tmIKtSyZlE3uFJELVlNIOLJ1OE=";

    #[test]
    fn signs_standard_webhooks_test_vector() {
        let key = decode_secret(SECRET).unwrap();
        assert_eq!(SIGNATURE, sign(&key, MSG_ID, TIMESTAMP, BODY.as_bytes()));
    }

    #[test]
    fn accepts_secret_without_prefix() {
        let key = decode_secret(SECRET.trim_start_matches(SECRET_PREFIX)).unwrap();
        assert_eq!(SIGNATURE, sign(&key, MSG_ID, TIMESTAMP, BODY.as_bytes()));
    }

    #[test]
    fn rejects_invalid_secrets() {
        assert!(decode_secret("").is_err());
        assert!(decode_secret("whsec_not base64!").is_err());
        assert!(decode_secret("whsec_c2hvcnQ=").is_err());
    }
}
