//! Serializable response types returned to the frontend, with `From` conversions
//! from the internal database models / auth types.

use serde::Serialize;

use crate::auth::TokenPair;
use crate::database::models::{dictionary::DictionaryEntry, transcript::Transcript, user::User};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserResponse {
    pub id: i64,
    pub email: String,
    pub created_at: String,
}

impl From<User> for UserResponse {
    fn from(value: User) -> Self {
        Self {
            id: value.id,
            email: value.email,
            created_at: value.created_at.to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptResponse {
    pub id: i64,
    pub content: String,
    pub word_count: i64,
    pub duration_seconds: Option<f64>,
    pub created_at: String,
}

impl From<Transcript> for TranscriptResponse {
    fn from(value: Transcript) -> Self {
        Self {
            id: value.id,
            content: value.content,
            word_count: value.word_count,
            duration_seconds: value.duration_seconds,
            created_at: value.created_at.to_string(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryResponse {
    pub id: i64,
    pub term: String,
    pub replacement: String,
    pub hits: i64,
    pub created_at: String,
}

impl From<DictionaryEntry> for DictionaryResponse {
    fn from(value: DictionaryEntry) -> Self {
        Self {
            id: value.id,
            term: value.term,
            replacement: value.replacement,
            hits: value.hits,
            created_at: value.created_at.to_string(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenPairResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in_seconds: i64,
}

impl From<TokenPair> for TokenPairResponse {
    fn from(p: TokenPair) -> Self {
        Self {
            access_token: p.access_token,
            refresh_token: p.refresh_token,
            expires_in_seconds: p.expires_in_seconds,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthResponse {
    pub user: UserResponse,
    pub tokens: TokenPairResponse,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthStateResponse {
    pub authenticated: bool,
    pub user_id: Option<i64>,
}
