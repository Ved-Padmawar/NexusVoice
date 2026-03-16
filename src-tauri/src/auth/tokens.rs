use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use rand::distr::Alphanumeric;
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::errors::AuthError;

pub const ACCESS_TOKEN_DAYS: i64 = 30;
pub const REFRESH_TOKEN_DAYS: i64 = 90;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub exp: i64,
    pub iat: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in_seconds: i64,
}

pub fn generate_access_token(user_id: i64, secret: &[u8]) -> Result<String, AuthError> {
    let now = Utc::now();
    let exp = (now + Duration::days(ACCESS_TOKEN_DAYS)).timestamp();
    let claims = Claims {
        sub: user_id.to_string(),
        exp,
        iat: now.timestamp(),
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret),
    )
    .map_err(|_| AuthError::TokenGeneration)
}

pub fn validate_access_token(token: &str, secret: &[u8]) -> Result<Claims, AuthError> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret),
        &Validation::default(),
    )
    .map(|data| data.claims)
    .map_err(|err| {
        use jsonwebtoken::errors::ErrorKind;
        match err.kind() {
            ErrorKind::ExpiredSignature => AuthError::TokenExpired,
            _ => AuthError::TokenInvalid,
        }
    })
}

pub fn generate_refresh_token() -> String {
    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(64)
        .map(char::from)
        .collect()
}

pub fn refresh_token_expires_at() -> String {
    (Utc::now() + Duration::days(REFRESH_TOKEN_DAYS))
        .naive_utc()
        .format("%Y-%m-%d %H:%M:%S")
        .to_string()
}

pub fn hash_token(raw: &str) -> String {
    let digest = Sha256::digest(raw.as_bytes());
    digest.iter().fold(String::new(), |mut acc, b| {
        use std::fmt::Write;
        write!(acc, "{b:02x}").ok();
        acc
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &[u8] = b"test-secret-key-for-unit-tests";

    #[test]
    fn access_token_round_trip() {
        let token = generate_access_token(42, SECRET).expect("generate");
        let claims = validate_access_token(&token, SECRET).expect("validate");
        assert_eq!(claims.sub, "42");
    }

    #[test]
    fn refresh_token_is_64_chars() {
        let token = generate_refresh_token();
        assert_eq!(token.len(), 64);
        assert!(token.chars().all(|c| c.is_alphanumeric()));
    }

    #[test]
    fn two_refresh_tokens_are_unique() {
        let a = generate_refresh_token();
        let b = generate_refresh_token();
        assert_ne!(a, b);
    }

    #[test]
    fn hash_token_is_deterministic() {
        let raw = "abc123";
        assert_eq!(hash_token(raw), hash_token(raw));
    }

    #[test]
    fn invalid_token_rejected() {
        let err = validate_access_token("not.a.token", SECRET).expect_err("should fail");
        assert!(matches!(err, AuthError::TokenInvalid));
    }
}
