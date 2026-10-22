use super::{
    common_prefix_len, from_ms, lead_speech_offset, normalize_word, trailing_overlap,
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
fn prompt_is_empty_while_the_window_still_covers_everything_committed() {
    // Prompting with text the window covers makes the model treat it as said
    // and emit only the continuation, silently shortening the hypothesis.
    let mut session = StreamingSession::new();
    session.set_committed(&["one", "two", "three"]);
    session.hyp_committed = 3;

    assert_eq!(session.prompt(), "");
}

#[test]
fn prompt_is_the_committed_text_that_scrolled_out_of_the_window() {
    let mut session = StreamingSession::new();
    session.set_committed(&["one", "two", "three", "four"]);
    session.hyp_committed = 1;

    assert_eq!(session.prompt(), "one two three");
}

#[test]
fn prompt_keeps_only_the_last_words() {
    let words: Vec<String> = (0..40).map(|i| i.to_string()).collect();
    let refs: Vec<&str> = words.iter().map(String::as_str).collect();
    let mut session = StreamingSession::new();
    session.set_committed(&refs);

    let tail = session.prompt();
    assert_eq!(tail.split_whitespace().count(), 30);
    assert!(tail.starts_with("10 "));
    assert!(tail.ends_with(" 39"));
}

// ── trailing_overlap ───────────────────────────────────────────────────
// The alignment that replaced counting words. Getting this wrong either drops
// the words a hypothesis did not restate or repeats the ones it did.

#[test]
fn overlap_is_zero_when_the_hypothesis_restates_nothing() {
    assert_eq!(
        trailing_overlap(&words(&["one", "two"]), &words(&["three"])),
        0
    );
}

#[test]
fn overlap_covers_a_hypothesis_that_restates_the_whole_window() {
    let committed = words(&["one", "two", "three"]);
    let hyp = words(&["two", "three", "four"]);
    assert_eq!(trailing_overlap(&committed, &hyp), 2);
}

#[test]
fn overlap_prefers_the_longest_match() {
    // "the" alone also matches, but the four-word run is the real seam.
    let committed = words(&["and", "the", "cat", "sat", "the"]);
    let hyp = words(&["the", "cat", "sat", "the", "mat"]);
    assert_eq!(trailing_overlap(&committed, &hyp), 4);
}

#[test]
fn overlap_survives_a_hypothesis_one_word_short() {
    // The old count-based skip shifted every later word for exactly this case.
    let committed = words(&["alpha", "beta", "gamma"]);
    assert_eq!(trailing_overlap(&committed, &words(&["gamma", "delta"])), 1);
}

#[test]
fn overlap_is_zero_against_empty_committed_text() {
    assert_eq!(trailing_overlap(&[], &words(&["one"])), 0);
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
    session.absorb();

    assert_eq!(session.text(), "");
    assert_eq!(session.hyp_committed, 0);
}

#[test]
fn agreeing_hypotheses_commit_their_shared_prefix() {
    let mut session = StreamingSession::new();
    session.segments = hypothesis(&[" the", " quick", " brown"], 1200);
    session.absorb();

    // Second decode agrees on "the quick" but revises the third word.
    session.segments = hypothesis(&[" the", " quick", " brownish", " fox"], 1600);
    session.absorb();

    assert_eq!(session.text(), "the quick");
    assert_eq!(session.hyp_committed, 2);
}

#[test]
fn committed_text_survives_a_disagreeing_decode() {
    let mut session = StreamingSession::new();
    session.segments = hypothesis(&[" one", " two"], 800);
    session.absorb();
    session.segments = hypothesis(&[" one", " two"], 800);
    session.absorb();
    assert_eq!(session.text(), "one two");

    // A later hypothesis that disagrees entirely past the committed prefix.
    session.segments = hypothesis(&[" one", " two", " zebra"], 1200);
    session.absorb();

    assert_eq!(
        session.text(),
        "one two",
        "committed text must never shrink"
    );
    assert_eq!(session.hyp_committed, 2);
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
        session.absorb();
    }

    // Each decode confirms the previous one's new word.
    assert_eq!(session.text(), "alpha beta gamma");
    assert_eq!(session.hyp_committed, 3);
}

#[test]
fn punctuation_attaches_to_the_committed_word() {
    let mut session = StreamingSession::new();
    session.segments = hypothesis(&[" hello", ",", " world"], 1200);
    session.absorb();
    session.segments = hypothesis(&[" hello", ",", " world"], 1200);
    session.absorb();

    assert_eq!(session.text(), "hello, world");
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
    session.set_committed(&["one", "two"]);
    session.hyp_committed = 2;
    let total_len = 20 * native_rate as usize;

    assert!(session.trim(total_len, native_rate));

    // Audio and hypothesis advance; the transcript is untouched.
    assert_eq!(session.text(), "one two");
    assert_eq!(session.window_start, from_ms(800, native_rate));
    assert_eq!(session.hyp_committed, 0);
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
    session.set_committed(&["one"]);
    session.hyp_committed = 1;

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
        session.absorb();
    }
    assert_eq!(session.text(), "one two three");
    assert_eq!(session.hyp_committed, 3);

    // The first segment is fully committed, so it can be cut away.
    assert!(session.trim(20 * native_rate as usize, native_rate));
    assert_eq!(session.window_start, from_ms(800, native_rate));
    assert_eq!(session.hyp_committed, 1, "only \"three\" is still in view");

    // Post-trim hypotheses are relative to the new window start, so "three"
    // leads and must not be committed twice.
    for _ in 0..2 {
        session.segments = hypothesis(&[" three", " four"], 800);
        session.absorb();
    }

    assert_eq!(session.text(), "one two three four");
}

#[test]
fn force_trim_cuts_at_a_committed_word_boundary() {
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
    session.set_committed(&["one", "two", "three", "four"]);
    session.hyp_committed = 4;
    let total_len = 20 * native_rate as usize;

    session.force_trim(total_len, native_rate);

    // A buffer cut only — the text was committed on agreement, not here.
    assert_eq!(session.text(), "one two three four");
    assert_eq!(session.window_start, from_ms(1600 * 10, native_rate));
    assert_eq!(session.hyp_committed, 0);
    assert_eq!(session.segments[0].words.len(), 1);
    assert_eq!(session.segments[0].words[0].text, " five");
}

#[test]
fn force_trim_is_a_noop_when_nothing_is_committed() {
    let mut session = StreamingSession::new();
    let native_rate = 16_000;
    session.segments = hypothesis(&[" one", " two"], 2000);
    session.hyp_committed = 0;

    session.force_trim(20 * native_rate as usize, native_rate);

    assert_eq!(session.window_start, 0);
}

#[test]
fn force_trim_is_a_noop_without_word_end_times() {
    let mut session = StreamingSession::new();
    let native_rate = 16_000;
    session.segments = vec![TimedSegment {
        words: vec![Word {
            text: " hello".to_string(),
            end_cs: None,
        }],
        end_ms: 2000,
    }];
    session.set_committed(&["hello"]);
    session.hyp_committed = 1;

    session.force_trim(20 * native_rate as usize, native_rate);

    assert_eq!(session.text(), "hello");
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
    session.set_committed(&["hello"]);
    session.hyp_committed = 1;

    session.force_trim(20 * native_rate as usize, native_rate);

    assert_eq!(session.window_start, 0);
}

// ── would_decode ───────────────────────────────────────────────────────
// The stream worker calls this before copying the window. If it never returns
// true, streaming silently stops producing partials; if it always does, every
// poll pays for a full re-decode.

#[test]
fn would_decode_is_false_before_enough_new_audio_arrives() {
    let session = StreamingSession::new();
    let rate = 16_000usize;
    let hz = 16_000u32;
    // 0.5 s — under MIN_NEW_AUDIO_SECS.
    assert!(!session.would_decode(rate / 2, hz));
}

#[test]
fn would_decode_is_true_once_a_second_of_new_audio_arrives() {
    let session = StreamingSession::new();
    let rate = 16_000usize;
    let hz = 16_000u32;
    assert!(session.would_decode(rate, hz));
    assert!(session.would_decode(rate * 5, hz));
}

#[test]
fn would_decode_measures_from_the_last_decode_not_the_buffer_start() {
    let mut session = StreamingSession::new();
    let rate = 16_000usize;
    let hz = 16_000u32;
    // Already decoded 10 s.
    session.decoded_len = rate * 10;

    // 10.5 s total: only 0.5 s is new.
    assert!(!session.would_decode(rate * 10 + rate / 2, hz));
    // 11 s total: a full second is new.
    assert!(session.would_decode(rate * 11, hz));
}

#[test]
fn would_decode_measures_from_the_window_start_after_a_trim() {
    // A trim advances window_start past decoded_len; the new audio is whatever
    // follows the later of the two, or a trim would trigger a decode for free.
    let mut session = StreamingSession::new();
    let rate = 16_000usize;
    let hz = 16_000u32;
    session.decoded_len = rate * 4;
    session.window_start = rate * 9;

    assert!(!session.would_decode(rate * 9, hz), "no new audio yet");
    assert!(session.would_decode(rate * 10, hz));
}

#[test]
fn would_decode_is_false_before_the_sample_rate_is_known() {
    // Rate 0 would divide by zero; the worker must just wait.
    assert!(!StreamingSession::new().would_decode(48_000, 0));
}

#[test]
fn would_decode_agrees_with_what_poll_actually_does() {
    // poll() re-derives the same condition. If the two drift, the worker either
    // skips a decode the session wanted or copies a window it throws away.
    let mut session = StreamingSession::new();
    let rate = 16_000usize;
    let hz = 16_000u32;
    session.decoded_len = rate * 3;
    for total in [0, rate / 2, rate - 1, rate, rate * 3, rate * 4, rate * 7] {
        let new_samples = total.saturating_sub(session.decoded_len.max(session.window_start));
        #[allow(clippy::cast_precision_loss)]
        let poll_would_decode = (new_samples as f64 / f64::from(hz)) >= 1.0;
        assert_eq!(
            session.would_decode(total, hz),
            poll_would_decode,
            "disagreed at total={total}"
        );
    }
}

// ── partial ────────────────────────────────────────────────────────────
// What the live pill renders: confirmed text plus the revisable tail.

#[test]
fn partial_is_empty_before_anything_is_decoded() {
    let (committed, tentative) = StreamingSession::new().partial();
    assert_eq!(committed, "");
    assert_eq!(tentative, "");
}

#[test]
fn partial_reports_an_undecided_hypothesis_as_entirely_tentative() {
    // One decode commits nothing, so the pill shows it all as provisional.
    let mut session = StreamingSession::new();
    session.segments = hypothesis(&[" hello", " world"], 800);
    session.absorb();

    let (committed, tentative) = session.partial();
    assert_eq!(committed, "");
    assert_eq!(tentative, " hello world");
}

#[test]
fn partial_splits_confirmed_text_from_the_revisable_tail() {
    let mut session = StreamingSession::new();
    session.segments = hypothesis(&[" the", " quick", " brown"], 1200);
    session.absorb();
    session.segments = hypothesis(&[" the", " quick", " brownish", " fox"], 1600);
    session.absorb();

    let (committed, tentative) = session.partial();
    assert_eq!(committed, "the quick");
    assert_eq!(
        tentative, " brownish fox",
        "only the unconfirmed tail is tentative"
    );
}

#[test]
fn partial_has_no_tentative_tail_when_the_whole_hypothesis_is_committed() {
    let mut session = StreamingSession::new();
    for _ in 0..2 {
        session.segments = hypothesis(&[" one", " two"], 800);
        session.absorb();
    }

    let (committed, tentative) = session.partial();
    assert_eq!(committed, "one two");
    assert_eq!(tentative, "", "nothing is still open to revision");
}

#[test]
fn partial_leads_the_tentative_tail_with_one_space() {
    // The pill concatenates the two halves, so the join must not fuse words.
    let mut session = StreamingSession::new();
    for _ in 0..2 {
        session.segments = hypothesis(&[" one"], 400);
        session.absorb();
    }
    session.segments = hypothesis(&[" one", " two"], 800);
    session.absorb();

    let (committed, tentative) = session.partial();
    assert_eq!(format!("{committed}{tentative}"), "one two");
}

// ── trim safety rail ───────────────────────────────────────────────────

#[test]
fn trim_refuses_a_cut_that_would_leave_too_little_audio() {
    // Decoders need at least MIN_WINDOW_SECS behind them; a cut this close to
    // the end of the buffer must be declined rather than starving the decode.
    let mut session = StreamingSession::new();
    let native_rate = 16_000;
    session.segments = vec![
        TimedSegment {
            words: vec![word(" one", 400)],
            end_ms: 9_500,
        },
        TimedSegment {
            words: vec![word(" two", 10_000)],
            end_ms: 10_000,
        },
    ];
    session.set_committed(&["one"]);
    session.hyp_committed = 1;

    // Buffer is 10 s; cutting at 9.5 s would leave 0.5 s.
    assert!(!session.trim(10 * native_rate as usize, native_rate));
    assert_eq!(session.window_start, 0, "window must not move");
    assert_eq!(session.segments.len(), 2);
}

// ── decode pacing ──────────────────────────────────────────────────────
// A decode that took longer raises the bar for the next one, so decoding costs
// a bounded share of real time on any model and any machine.

#[test]
fn a_fast_decode_leaves_the_gate_at_the_floor() {
    let mut session = StreamingSession::new();
    session.last_decode_secs = 0.2;

    assert!((session.decode_gate_secs() - 1.0).abs() < f64::EPSILON);
}

#[test]
fn a_slow_decode_widens_the_gate_to_hold_the_duty_cycle() {
    let mut session = StreamingSession::new();
    session.last_decode_secs = 1.5;

    // 1.5 s of decode per 3.0 s of audio is the 50% the pipeline is allowed.
    assert!((session.decode_gate_secs() - 3.0).abs() < f64::EPSILON);
}

#[test]
fn the_gate_is_capped_so_a_very_slow_model_still_refreshes() {
    let mut session = StreamingSession::new();
    session.last_decode_secs = 30.0;

    assert!((session.decode_gate_secs() - 5.0).abs() < f64::EPSILON);
    assert!(session.would_decode(6 * 16_000, 16_000));
}
