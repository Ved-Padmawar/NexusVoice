use sqlx::sqlite::SqlitePoolOptions;

use super::connection::init_database;
use super::dto::{dictionary::CreateDictionaryEntry, transcript::CreateTranscript};
use super::repositories::{dictionary::DictionaryRepository, transcript::TranscriptRepository};

#[tokio::test]
async fn database_roundtrip() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("pool");
    init_database(&pool).await.expect("migrations");

    let transcripts = TranscriptRepository::new(pool.clone());
    let dictionary = DictionaryRepository::new(pool.clone());

    let transcript = transcripts
        .create(CreateTranscript {
            content: "hello".to_string(),
            word_count: 1,
            duration_seconds: None,
            target_app: Some("VS Code".to_string()),
        })
        .await
        .expect("create transcript");

    let fetched_transcript = transcripts
        .get_by_id(transcript.id)
        .await
        .expect("get transcript")
        .expect("transcript exists");

    assert_eq!(fetched_transcript.content, "hello");
    assert_eq!(fetched_transcript.target_app.as_deref(), Some("VS Code"));

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
