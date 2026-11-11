use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;

use super::{Dictionary, DictionaryError};
use crate::data::connection::init_database;

async fn pool() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("pool");
    init_database(&pool).await.expect("migrations");
    pool
}

#[tokio::test]
async fn blank_and_overlong_entries_are_rejected_and_others_trimmed() {
    let pool = pool().await;
    let dictionary = Dictionary::default();

    for (term, replacement) in [("  ", "Zoom"), ("zoom", " "), ("a b c d", "x")] {
        let result = dictionary.save(&pool, None, term, replacement).await;
        assert!(
            matches!(result, Err(DictionaryError::Invalid(_))),
            "{term:?}"
        );
    }
    let saved = dictionary
        .save(&pool, None, " zoom ", " Zoom ")
        .await
        .expect("save");
    assert_eq!(
        (saved.term.as_str(), saved.replacement.as_str()),
        ("zoom", "Zoom")
    );
}

#[tokio::test]
async fn a_term_matching_another_entry_is_a_duplicate_but_a_rename_keeps_its_row() {
    let pool = pool().await;
    let dictionary = Dictionary::default();
    let next = dictionary
        .save(&pool, None, "next js", "Next.js")
        .await
        .expect("first");
    dictionary
        .save(&pool, None, "zoom", "Zoom")
        .await
        .expect("second");

    let Err(DictionaryError::Duplicate(message)) =
        dictionary.save(&pool, None, "NextJS", "Next.js").await
    else {
        panic!("a differently spaced and cased term is the same entry");
    };
    assert_eq!(message, "“NextJS” is already in your dictionary.");
    assert!(matches!(
        dictionary.save(&pool, Some(next.id), "ZOOM", "Zoom").await,
        Err(DictionaryError::Duplicate(_))
    ));

    let renamed = dictionary
        .save(&pool, Some(next.id), "Next JS", "Next.js")
        .await
        .expect("rename");
    assert_eq!(renamed.id, next.id);
    assert_eq!(dictionary.entries().await.len(), 2);
}
