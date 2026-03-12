pub mod connection;
pub mod dto;
pub mod models;
pub mod repositories;

#[cfg(test)]
mod tests {
    use sqlx::sqlite::SqlitePoolOptions;

    use super::connection::init_database;
    use super::dto::{
        dictionary::CreateDictionaryEntry, transcript::CreateTranscript, user::CreateUser,
    };
    use super::repositories::{
        dictionary::DictionaryRepository, transcript::TranscriptRepository, user::UserRepository,
    };

    #[tokio::test]
    async fn database_roundtrip() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("pool");
        init_database(&pool).await.expect("migrations");

        let users = UserRepository::new(pool.clone());
        let transcripts = TranscriptRepository::new(pool.clone());
        let dictionary = DictionaryRepository::new(pool.clone());

        let user = users
            .create(CreateUser {
                email: "user@example.com".to_string(),
                password_hash: "hash".to_string(),
            })
            .await
            .expect("create user");

        let fetched = users
            .get_by_email("user@example.com")
            .await
            .expect("get by email")
            .expect("user exists");

        assert_eq!(user.id, fetched.id);

        let transcript = transcripts
            .create(CreateTranscript {
                content: "hello".to_string(),
            })
            .await
            .expect("create transcript");

        let fetched_transcript = transcripts
            .get_by_id(transcript.id)
            .await
            .expect("get transcript")
            .expect("transcript exists");

        assert_eq!(fetched_transcript.content, "hello");

        let entry = dictionary
            .create(CreateDictionaryEntry {
                term: "teh".to_string(),
                replacement: "the".to_string(),
            })
            .await
            .expect("create entry");

        let fetched_entry = dictionary
            .get_by_term("teh")
            .await
            .expect("get entry")
            .expect("entry exists");

        assert_eq!(entry.id, fetched_entry.id);
    }
}
