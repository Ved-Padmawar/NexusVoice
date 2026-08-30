use super::{
    common_prefix_len, from_ms, lead_speech_offset, normalize_word, prompt_tail, push_text,
    StreamingSession, VAD_CHUNK_16K, VAD_PAD_FRAMES,
};
use crate::inference::transcript::{TimedSegment, Word};

fn word(text: &str, end_cs: i64) -> Word {
    Word {
        text: text.to_string(),
        end_cs: Some(end_cs),
    }
}

fn words(list: &[&str]) -> Vec<String> {
    list.iter().map(|w| normalize_word(w)).collect()
}

/// Voiced-sounding tone stack (fundamental + harmonics, syllable-rate envelope).
/// A pure sine reads as noise to the detector; harmonics make it score as speech.
fn voiced(samples: usize) -> Vec<f32> {
    (0..samples)
        .map(|i| {
            #[allow(clippy::cast_precision_loss)]
            let t = i as f32 / 16_000.0;
            let env = (2.0 * std::f32::consts::PI * 4.0 * t).sin().abs();
            let tone = (2.0 * std::f32::consts::PI * 220.0 * t).sin() * 0.5
                + (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.3
                + (2.0 * std::f32::consts::PI * 880.0 * t).sin() * 0.2;
            tone * env * 0.6
        })
        .collect()
}

#[test]
fn agreement_ignores_case_and_punctuation() {
    // Whisper flips "Okay," ↔ "okay" between decodes; that must still agree.
    let a = words(&["Okay,", "so", "we", "start"]);
    let b = words(&["okay", "so", "we", "started"]);
    assert_eq!(common_prefix_len(&a, &b), 3);
}

#[test]
fn agreement_is_empty_on_disjoint_hypotheses() {
    let a = words(&["hello", "world"]);
    let b = words(&["goodbye", "world"]);
    assert_eq!(common_prefix_len(&a, &b), 0);
}

#[test]
fn agreement_handles_unequal_lengths() {
    let a = words(&["one", "two"]);
    let b = words(&["one", "two", "three"]);
    assert_eq!(common_prefix_len(&a, &b), 2);
    assert_eq!(common_prefix_len(&b, &a), 2);
}

#[test]
fn lead_speech_offset_is_none_for_pure_silence() {
    // Nothing to trim to, so the caller keeps the whole buffer.
    assert_eq!(lead_speech_offset(&vec![0.0; 16_000]), None);
}

#[test]
fn lead_speech_offset_is_none_for_a_buffer_shorter_than_one_frame() {
    assert_eq!(lead_speech_offset(&vec![0.0; VAD_CHUNK_16K - 1]), None);
}

#[test]
fn lead_speech_offset_trims_leading_silence_back_by_the_pad() {
    // 0.5 s silence, then speech: onset sits on a frame boundary.
    let lead_frames = 31;
    let mut buf = vec![0.0f32; lead_frames * VAD_CHUNK_16K];
    buf.extend(voiced(8_000));

    let offset = lead_speech_offset(&buf).expect("speech should be detected");

    // Padding must land the cut before the onset so no speech is clipped, but
    // still inside the silence rather than back at zero.
    let onset = lead_frames * VAD_CHUNK_16K;
    assert!(offset < onset, "offset {offset} must not clip the onset");
    assert_eq!(offset, onset - VAD_PAD_FRAMES * VAD_CHUNK_16K);
}

#[test]
fn lead_speech_offset_is_zero_when_speech_starts_immediately() {
    // Saturating pad: no silence to trim, so the buffer is kept whole.
    let buf = voiced(8_000);
    assert_eq!(lead_speech_offset(&buf), Some(0));
}

#[test]
fn normalize_strips_everything_but_alphanumerics() {
    assert_eq!(normalize_word("Okay,"), "okay");
    assert_eq!(normalize_word("it's"), "its");
    assert_eq!(normalize_word("—"), "");
}

#[test]
fn prompt_tail_keeps_only_the_last_words() {
    let committed = (0..40).map(|i| i.to_string()).collect::<Vec<_>>().join(" ");
    let tail = prompt_tail(&committed);
    assert_eq!(tail.split_whitespace().count(), 30);
    assert!(tail.starts_with("10 "));
    assert!(tail.ends_with(" 39"));
}

#[test]
fn prompt_tail_of_empty_text_is_empty() {
    assert_eq!(prompt_tail(""), "");
}

#[test]
fn push_text_separates_with_a_single_space() {
    let mut out = String::new();
    push_text(&mut out, "hello");
    push_text(&mut out, " world ");
    push_text(&mut out, "");
    assert_eq!(out, "hello world");
}

#[test]
fn from_ms_maps_to_native_rate() {
    assert_eq!(from_ms(1000, 48_000), 48_000);
    assert_eq!(from_ms(500, 44_100), 22_050);
    assert_eq!(from_ms(-5, 48_000), 0);
}

/// Build a hypothesis of `texts` as one segment, 400 cs apart.
fn hypothesis(texts: &[&str], end_ms: i64) -> Vec<TimedSegment> {
    vec![TimedSegment {
        words: texts
            .iter()
            .enumerate()
            .map(|(i, t)| word(t, i64::try_from(i).expect("test index") * 400 + 400))
            .collect(),
        end_ms,
    }]
}

#[test]
fn nothing_is_committed_from_a_single_hypothesis() {
    // LocalAgreement-2 needs two decodes to agree before anything is confirmed.
    let mut session = StreamingSession::new();
    session.segments = hypothesis(&[" hello", " world"], 800);
    session.commit_agreed();

    assert_eq!(session.committed, "");
    assert_eq!(session.committed_words, 0);
}

#[test]
fn agreeing_hypotheses_commit_their_shared_prefix() {
    let mut session = StreamingSession::new();
    session.segments = hypothesis(&[" the", " quick", " brown"], 1200);
    session.commit_agreed();

    // Second decode agrees on "the quick" but revises the third word.
    session.segments = hypothesis(&[" the", " quick", " brownish", " fox"], 1600);
    session.commit_agreed();

    assert_eq!(session.committed, "the quick");
    assert_eq!(session.committed_words, 2);
}

#[test]
fn committed_text_survives_a_disagreeing_decode() {
    let mut session = StreamingSession::new();
    session.segments = hypothesis(&[" one", " two"], 800);
    session.commit_agreed();
    session.segments = hypothesis(&[" one", " two"], 800);
    session.commit_agreed();
    assert_eq!(session.committed, "one two");

    // A later hypothesis that disagrees entirely past the committed prefix.
    session.segments = hypothesis(&[" one", " two", " zebra"], 1200);
    session.commit_agreed();

    assert_eq!(
        session.committed, "one two",
        "committed text must never shrink"
    );
    assert_eq!(session.committed_words, 2);
}

#[test]
fn commits_accumulate_across_successive_agreements() {
    let mut session = StreamingSession::new();
    for texts in [
        &[" alpha"][..],
        &[" alpha", " beta"][..],
        &[" alpha", " beta", " gamma"][..],
        &[" alpha", " beta", " gamma", " delta"][..],
    ] {
        let end_ms = i64::try_from(texts.len()).expect("test length") * 400;
        session.segments = hypothesis(texts, end_ms);
        session.commit_agreed();
    }

    // Each decode confirms the previous one's new word.
    assert_eq!(session.committed, "alpha beta gamma");
    assert_eq!(session.committed_words, 3);
}

#[test]
fn punctuation_attaches_to_the_committed_word() {
    let mut session = StreamingSession::new();
    session.segments = hypothesis(&[" hello", ",", " world"], 1200);
    session.commit_agreed();
    session.segments = hypothesis(&[" hello", ",", " world"], 1200);
    session.commit_agreed();

    assert_eq!(session.committed, "hello, world");
}

#[test]
fn trim_cuts_at_a_committed_segment_boundary_without_touching_text() {
    let mut session = StreamingSession::new();
    let native_rate = 16_000;
    session.segments = vec![
        TimedSegment {
            words: vec![word(" one", 400), word(" two", 800)],
            end_ms: 800,
        },
        TimedSegment {
            words: vec![word(" three", 1200)],
            end_ms: 1200,
        },
    ];
    session.committed = "one two".to_string();
    session.committed_words = 2;
    let total_len = 20 * native_rate as usize;

    assert!(session.trim(total_len, native_rate));

    // Audio and hypothesis advance; the transcript is untouched.
    assert_eq!(session.committed, "one two");
    assert_eq!(session.window_start, from_ms(800, native_rate));
    assert_eq!(session.committed_words, 0);
    assert_eq!(session.segments.len(), 1);
}

#[test]
fn trim_is_a_noop_when_no_segment_is_fully_committed() {
    let mut session = StreamingSession::new();
    let native_rate = 16_000;
    session.segments = vec![
        TimedSegment {
            words: vec![word(" one", 400), word(" two", 800)],
            end_ms: 800,
        },
        TimedSegment {
            words: vec![word(" three", 1200)],
            end_ms: 1200,
        },
    ];
    session.committed = "one".to_string();
    session.committed_words = 1;

    assert!(!session.trim(20 * native_rate as usize, native_rate));
    assert_eq!(session.window_start, 0);
}

#[test]
fn committing_continues_across_a_trim() {
    let mut session = StreamingSession::new();
    let native_rate = 16_000;

    // Two identical decodes commit the whole hypothesis.
    for _ in 0..2 {
        session.segments = vec![
            TimedSegment {
                words: vec![word(" one", 400), word(" two", 800)],
                end_ms: 800,
            },
            TimedSegment {
                words: vec![word(" three", 1200)],
                end_ms: 1200,
            },
        ];
        session.commit_agreed();
    }
    assert_eq!(session.committed, "one two three");
    assert_eq!(session.committed_words, 3);

    // The first segment is fully committed, so it can be cut away.
    assert!(session.trim(20 * native_rate as usize, native_rate));
    assert_eq!(session.window_start, from_ms(800, native_rate));
    assert_eq!(
        session.committed_words, 1,
        "only \"three\" is still in view"
    );

    // Post-trim hypotheses are relative to the new window start, so "three"
    // leads and must not be committed twice.
    for _ in 0..2 {
        session.segments = hypothesis(&[" three", " four"], 800);
        session.commit_agreed();
    }

    assert_eq!(session.committed, "one two three four");
}

#[test]
fn force_trim_cuts_a_single_growing_segment_at_a_word_boundary() {
    let mut session = StreamingSession::new();
    let native_rate = 16_000;
    session.segments = vec![TimedSegment {
        words: vec![
            word(" one", 400),
            word(" two", 800),
            word(" three", 1200),
            word(" four", 1600),
            word(" five", 2000),
        ],
        end_ms: 2000,
    }];
    session.committed = "one two three four".to_string();
    session.committed_words = 4;
    let total_len = 20 * native_rate as usize;

    session.force_trim(total_len, native_rate);

    // A buffer cut only — the text was committed on agreement, not here.
    assert_eq!(session.committed, "one two three four");
    assert_eq!(session.window_start, from_ms(1600 * 10, native_rate));
    assert_eq!(session.committed_words, 0);
    assert_eq!(session.segments[0].words.len(), 1);
    assert_eq!(session.segments[0].words[0].text, " five");
}

#[test]
fn force_trim_is_a_noop_when_nothing_is_committed() {
    let mut session = StreamingSession::new();
    let native_rate = 16_000;
    session.segments = hypothesis(&[" one", " two"], 2000);
    session.committed_words = 0;

    session.force_trim(20 * native_rate as usize, native_rate);

    assert_eq!(session.window_start, 0);
}

#[test]
fn force_trim_is_a_noop_without_dtw_timestamps() {
    let mut session = StreamingSession::new();
    let native_rate = 16_000;
    session.segments = vec![TimedSegment {
        words: vec![Word {
            text: " hello".to_string(),
            end_cs: None,
        }],
        end_ms: 2000,
    }];
    session.committed = "hello".to_string();
    session.committed_words = 1;

    session.force_trim(20 * native_rate as usize, native_rate);

    assert_eq!(session.committed, "hello");
    assert_eq!(session.window_start, 0);
}

#[test]
fn force_trim_is_a_noop_when_it_would_leave_too_little_window() {
    let mut session = StreamingSession::new();
    let native_rate = 16_000;
    session.segments = vec![TimedSegment {
        words: vec![word(" hello", 1990)],
        end_ms: 2000,
    }];
    session.committed = "hello".to_string();
    session.committed_words = 1;

    session.force_trim(20 * native_rate as usize, native_rate);

    assert_eq!(session.window_start, 0);
}
