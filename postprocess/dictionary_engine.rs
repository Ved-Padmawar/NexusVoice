use crate::database::models::dictionary::DictionaryEntry;
use crate::database::repositories::dictionary::DictionaryRepository;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct DictionaryCorrectionConfig {
    pub max_distance: usize,
    pub candidate_limit: i64,
    pub prefix_len: usize,
}

impl Default for DictionaryCorrectionConfig {
    fn default() -> Self {
        Self {
            max_distance: 2,
            candidate_limit: 500,
            prefix_len: 2,
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorrectionResult {
    pub term: String,
    pub replacement: String,
    pub distance: usize,
    pub exact: bool,
}

#[allow(dead_code)]
#[derive(Clone)]
pub struct DictionaryCorrectionEngine {
    repository: DictionaryRepository,
    config: DictionaryCorrectionConfig,
}

#[allow(dead_code)]
impl DictionaryCorrectionEngine {
    pub fn new(repository: DictionaryRepository, config: DictionaryCorrectionConfig) -> Self {
        Self { repository, config }
    }

    pub fn with_default_config(repository: DictionaryRepository) -> Self {
        Self {
            repository,
            config: DictionaryCorrectionConfig::default(),
        }
    }

    /// Apply dictionary corrections to a full text string word-by-word.
    /// Words that match dictionary entries (exact or fuzzy) are replaced.
    /// Punctuation attached to words is preserved.
    pub async fn apply_to_text(&self, text: &str) -> Result<String, sqlx::Error> {
        let mut result = Vec::new();
        for token in text.split_whitespace() {
            // Separate leading/trailing punctuation from the word core
            let start = token
                .find(|c: char| c.is_alphabetic())
                .unwrap_or(token.len());
            let end = token
                .rfind(|c: char| c.is_alphabetic())
                .map(|i| i + token[i..].chars().next().map_or(0, |ch| ch.len_utf8()))
                .unwrap_or(0);

            if start >= end {
                result.push(token.to_string());
                continue;
            }

            let prefix = &token[..start];
            let word = &token[start..end];
            let suffix = &token[end..];

            let corrected = match self.correct(word).await? {
                Some(c) => c.replacement,
                None => word.to_string(),
            };
            result.push(format!("{prefix}{corrected}{suffix}"));
        }
        Ok(result.join(" "))
    }

    pub async fn correct(&self, input: &str) -> Result<Option<CorrectionResult>, sqlx::Error> {
        if let Some(entry) = self.repository.get_by_term(input).await? {
            return Ok(Some(CorrectionResult {
                term: entry.term,
                replacement: entry.replacement,
                distance: 0,
                exact: true,
            }));
        }

        let prefix_len = self.config.prefix_len.min(input.len());
        let entries = if prefix_len == 0 {
            self.repository.list_all().await?
        } else {
            let prefix = &input[..prefix_len];
            let candidates = self
                .repository
                .list_candidates(prefix, self.config.candidate_limit)
                .await?;
            if candidates.is_empty() {
                self.repository.list_all().await?
            } else {
                candidates
            }
        };
        let mut best: Option<(DictionaryEntry, usize)> = None;

        for entry in entries {
            let distance = levenshtein(input, &entry.term);
            if distance > self.config.max_distance {
                continue;
            }

            match &best {
                Some((_, best_distance)) if distance >= *best_distance => {}
                _ => best = Some((entry, distance)),
            }
        }

        Ok(best.map(|(entry, distance)| CorrectionResult {
            term: entry.term,
            replacement: entry.replacement,
            distance,
            exact: false,
        }))
    }
}

#[allow(dead_code)]
fn levenshtein(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();

    let mut prev_row: Vec<usize> = (0..=b_chars.len()).collect();
    let mut curr_row = vec![0; b_chars.len() + 1];

    for (i, a_char) in a_chars.iter().enumerate() {
        curr_row[0] = i + 1;
        for (j, b_char) in b_chars.iter().enumerate() {
            let cost = if a_char == b_char { 0 } else { 1 };
            curr_row[j + 1] = std::cmp::min(
                std::cmp::min(curr_row[j] + 1, prev_row[j + 1] + 1),
                prev_row[j] + cost,
            );
        }
        prev_row.clone_from_slice(&curr_row);
    }

    prev_row[b_chars.len()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    use crate::database::connection::init_database;
    use crate::database::dto::dictionary::CreateDictionaryEntry;
    use crate::database::repositories::dictionary::DictionaryRepository;

    #[tokio::test]
    async fn exact_match_wins() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("pool");
        init_database(&pool).await.expect("migrations");

        let repo = DictionaryRepository::new(pool);
        repo.create(CreateDictionaryEntry {
            term: "teh".to_string(),
            replacement: "the".to_string(),
        })
        .await
        .expect("create");

        let engine = DictionaryCorrectionEngine::with_default_config(repo);
        let result = engine.correct("teh").await.expect("correct").expect("hit");

        assert!(result.exact);
        assert_eq!(result.replacement, "the");
        assert_eq!(result.distance, 0);
    }

    #[tokio::test]
    async fn fuzzy_match_returns_best() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("pool");
        init_database(&pool).await.expect("migrations");

        let repo = DictionaryRepository::new(pool);
        repo.create(CreateDictionaryEntry {
            term: "receive".to_string(),
            replacement: "receive".to_string(),
        })
        .await
        .expect("create");

        repo.create(CreateDictionaryEntry {
            term: "recieve".to_string(),
            replacement: "receive".to_string(),
        })
        .await
        .expect("create");

        let engine = DictionaryCorrectionEngine::with_default_config(repo);
        let result = engine
            .correct("recive")
            .await
            .expect("correct")
            .expect("hit");

        assert!(!result.exact);
        assert_eq!(result.replacement, "receive");
    }

    #[tokio::test]
    async fn apply_to_text_corrects_words() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("pool");
        init_database(&pool).await.expect("migrations");

        let repo = DictionaryRepository::new(pool);
        repo.create(CreateDictionaryEntry {
            term: "teh".to_string(),
            replacement: "the".to_string(),
        })
        .await
        .expect("create");
        repo.create(CreateDictionaryEntry {
            term: "gonna".to_string(),
            replacement: "going to".to_string(),
        })
        .await
        .expect("create");

        let engine = DictionaryCorrectionEngine::with_default_config(repo);
        let result = engine
            .apply_to_text("teh dog is gonna run")
            .await
            .expect("apply");

        assert_eq!(result, "the dog is going to run");
    }

    #[tokio::test]
    async fn apply_to_text_preserves_punctuation() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("pool");
        init_database(&pool).await.expect("migrations");

        let repo = DictionaryRepository::new(pool);
        repo.create(CreateDictionaryEntry {
            term: "teh".to_string(),
            replacement: "the".to_string(),
        })
        .await
        .expect("create");

        let engine = DictionaryCorrectionEngine::with_default_config(repo);
        let result = engine.apply_to_text("teh, dog.").await.expect("apply");

        assert_eq!(result, "the, dog.");
    }

    #[tokio::test]
    async fn no_match_returns_none() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("pool");
        init_database(&pool).await.expect("migrations");

        let repo = DictionaryRepository::new(pool);
        let engine = DictionaryCorrectionEngine::new(
            repo,
            DictionaryCorrectionConfig {
                max_distance: 1,
                candidate_limit: 100,
                prefix_len: 2,
            },
        );

        let result = engine.correct("unknown").await.expect("correct");
        assert!(result.is_none());
    }
}
