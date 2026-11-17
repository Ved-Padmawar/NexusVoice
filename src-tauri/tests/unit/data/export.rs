//! An export's file: what goes in it, and where.

use super::*;
use crate::data::connection::init_database;
use crate::data::dto::transcript::CreateTranscript;
use sqlx::sqlite::SqlitePoolOptions;

#[test]
fn an_existing_export_is_never_overwritten() {
    let dir = tempfile::tempdir().expect("tempdir");
    let names: Vec<_> = (0..3)
        .map(|_| {
            let (path, _) = create_without_overwriting(dir.path(), "words.json").expect("create");
            path.file_name().unwrap().to_string_lossy().into_owned()
        })
        .collect();

    assert_eq!(names, ["words.json", "words (2).json", "words (3).json"]);
}

#[tokio::test]
async fn an_export_holds_every_finished_row_it_was_asked_for() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("pool");
    init_database(&pool).await.expect("migrations");
    let repo = TranscriptRepository::new(pool);
    let mut ids = Vec::new();
    for n in 0..4 {
        let row = repo
            .create(CreateTranscript {
                content: format!("entry {n}"),
                word_count: 2,
                duration_seconds: None,
                target_app: None,
                source_name: None,
                model: None,
                pending: n == 0,
                language: None,
            })
            .await
            .expect("create");
        ids.push(row.id);
    }
    let dir = tempfile::tempdir().expect("tempdir");
    let selection = |exclude: HashSet<i64>| Selection {
        search: None,
        from: None,
        to: None,
        sort_desc: true,
        exclude,
    };

    let (path, count) = export(
        &repo,
        selection(HashSet::from([ids[1]])),
        dir.path().to_path_buf(),
        ExportFormat::Json,
    )
    .await
    .expect("export")
    .expect("rows matched");

    let items: Vec<serde_json::Value> =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let exported: Vec<i64> = items.iter().map(|i| i["id"].as_i64().unwrap()).collect();
    let expected: Vec<i64> = ids[2..].iter().rev().copied().collect();
    assert_eq!(count, expected.len());
    assert_eq!(exported, expected);

    // Nothing left to export makes no file.
    let none = export(
        &repo,
        selection(ids.iter().copied().collect()),
        dir.path().to_path_buf(),
        ExportFormat::Txt,
    )
    .await
    .expect("export");
    assert!(none.is_none());
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}
