use super::{
    common_prefix_len, final_speech_offset, from_ms, last_covered_pause, lead_speech_offset,
    normalize_word, trailing_overlap, LeadingSpeech, StreamingSession, VAD_CHUNK_16K,
    VAD_PAD_FRAMES,
};
use crate::models::transcript::{TimedSegment, Word};

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
        end_ms: Some(end_ms),
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
            end_ms: Some(800),
        },
        TimedSegment {
            words: vec![word(" three", 1200)],
            end_ms: Some(1200),
        },
    ];
    session.set_committed(&["one", "two"]);
    session.hyp_committed = 2;
    let total_len = 20 * native_rate as usize;

    assert!(session.trim_at_segment(total_len, native_rate));

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
            end_ms: Some(800),
        },
        TimedSegment {
            words: vec![word(" three", 1200)],
            end_ms: Some(1200),
        },
    ];
    session.set_committed(&["one"]);
    session.hyp_committed = 1;

    assert!(!session.trim_at_segment(20 * native_rate as usize, native_rate));
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
                end_ms: Some(800),
            },
            TimedSegment {
                words: vec![word(" three", 1200)],
                end_ms: Some(1200),
            },
        ];
        session.absorb();
    }
    assert_eq!(session.text(), "one two three");
    assert_eq!(session.hyp_committed, 3);

    // The first segment is fully committed, so it can be cut away.
    assert!(session.trim_at_segment(20 * native_rate as usize, native_rate));
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
fn word_trim_cuts_at_a_committed_word_boundary() {
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
        end_ms: Some(2000),
    }];
    session.set_committed(&["one", "two", "three", "four"]);
    session.hyp_committed = 4;
    let total_len = 20 * native_rate as usize;

    assert!(session.trim_at_word(total_len, native_rate));

    // A buffer cut only — the text was committed on agreement, not here.
    assert_eq!(session.text(), "one two three four");
    assert_eq!(session.window_start, from_ms(1600 * 10, native_rate));
    assert_eq!(session.hyp_committed, 0);
    assert_eq!(session.segments[0].words.len(), 1);
    assert_eq!(session.segments[0].words[0].text, " five");
}

#[test]
fn word_trim_is_a_noop_when_nothing_is_committed() {
    let mut session = StreamingSession::new();
    let native_rate = 16_000;
    session.segments = hypothesis(&[" one", " two"], 2000);
    session.hyp_committed = 0;

    assert!(!session.trim_at_word(20 * native_rate as usize, native_rate));

    assert_eq!(session.window_start, 0);
}

#[test]
fn word_trim_is_a_noop_without_word_end_times() {
    let mut session = StreamingSession::new();
    let native_rate = 16_000;
    session.segments = vec![TimedSegment {
        words: vec![Word {
            text: " hello".to_string(),
            end_cs: None,
        }],
        end_ms: Some(2000),
    }];
    session.set_committed(&["hello"]);
    session.hyp_committed = 1;

    assert!(!session.trim_at_word(20 * native_rate as usize, native_rate));

    assert_eq!(session.text(), "hello");
    assert_eq!(session.window_start, 0);
}

#[test]
fn word_trim_is_a_noop_when_it_would_leave_too_little_window() {
    let mut session = StreamingSession::new();
    let native_rate = 16_000;
    session.segments = vec![TimedSegment {
        words: vec![word(" hello", 1990)],
        end_ms: Some(2000),
    }];
    session.set_committed(&["hello"]);
    session.hyp_committed = 1;

    assert!(!session.trim_at_word(20 * native_rate as usize, native_rate));

    assert_eq!(session.window_start, 0);
}

// ── models without timestamps ──────────────────────────────────────────

/// Speech, a one second pause, then more speech: a 13 s window whose only
/// cuttable pause sits at 4.5 s, outside the audio a pause cut must keep.
fn window_with_a_pause() -> Vec<f32> {
    let mut audio = voiced(4 * 16_000);
    audio.extend(vec![0.0; 16_000]);
    audio.extend(voiced(8 * 16_000));
    audio
}

/// A session holding `window_with_a_pause` whose hypothesis carries no times.
fn untimed_session() -> StreamingSession {
    let mut session = StreamingSession::new();
    session.window_16k = window_with_a_pause();
    session.fed = session.window_16k.len();
    session.segments = vec![TimedSegment {
        words: vec![
            Word {
                text: " one".to_string(),
                end_cs: None,
            },
            Word {
                text: " two".to_string(),
                end_cs: None,
            },
        ],
        end_ms: None,
    }];
    session.set_committed(&["one", "two"]);
    session.hyp_committed = 2;
    session.prev_norm = words(&["three"]);
    session
}

#[test]
fn segment_trim_skips_segments_without_end_times() {
    // An untimed model reports every segment at 0 ms; cutting there would
    // claim a boundary that does not exist.
    let mut session = untimed_session();
    session.segments.push(session.segments[0].clone());

    assert!(!session.trim_at_segment(13 * 16_000, 16_000));
    assert_eq!(session.window_start, 0);
}

#[test]
fn last_covered_pause_is_none_in_unbroken_speech() {
    assert_eq!(last_covered_pause(&voiced(4 * 16_000), 2, 2), None);
}

#[test]
fn last_covered_pause_lands_inside_the_silence() {
    let pause =
        last_covered_pause(&window_with_a_pause(), 2, 2).expect("the pause should be found");

    assert!(
        (4 * 16_000..5 * 16_000).contains(&pause),
        "cut at {pause} is outside the silence"
    );
}

#[test]
fn pause_trim_cuts_in_the_silence_and_keeps_the_unconfirmed_tail() {
    let mut session = untimed_session();
    let before = session.window_16k.len();

    assert!(session.trim_at_pause(16_000));

    assert!((4 * 16_000..5 * 16_000).contains(&session.window_start));
    assert_eq!(session.window_16k.len(), before - session.window_start);
    assert_eq!(session.segments.len(), 0);
    assert_eq!(session.prev_norm, words(&["three"]));
    assert_eq!(session.hyp_committed, 0);
    assert_eq!(session.text(), "one two", "a buffer cut never touches text");
}

#[test]
fn pause_trim_stays_within_the_committed_text() {
    // Half the words are committed, so the pause a third of the way through the
    // speech may hold words nobody has confirmed yet.
    let mut session = untimed_session();
    session.hyp_committed = 1;

    assert!(!session.trim_at_pause(16_000));
    assert_eq!(session.window_start, 0);
    assert_eq!(session.hyp_committed, 1);
}

#[test]
fn a_long_untimed_window_is_cut_at_a_pause() {
    // No segment or word times: the chain reaches the pause cut on its own.
    let mut session = untimed_session();

    session.trim(13 * 16_000, 16_000);

    assert!((4 * 16_000..5 * 16_000).contains(&session.window_start));
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

// ── partial ────────────────────────────────────────────────────────────
// What the live pill renders: confirmed text plus the revisable tail.

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
            end_ms: Some(9_500),
        },
        TimedSegment {
            words: vec![word(" two", 10_000)],
            end_ms: Some(10_000),
        },
    ];
    session.set_committed(&["one"]);
    session.hyp_committed = 1;

    // Buffer is 10 s; cutting at 9.5 s would leave 0.5 s.
    assert!(!session.trim_at_segment(10 * native_rate as usize, native_rate));
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

// ── audio window ───────────────────────────────────────────────────────

#[test]
fn ingest_converts_each_piece_once_and_counts_it_as_fed() {
    let mut session = StreamingSession::new();
    let rate = 48_000;
    session.ingest(&vec![0.1; rate as usize], rate).unwrap();
    session.ingest(&vec![0.1; rate as usize], rate).unwrap();

    assert_eq!(session.fed(), 2 * rate as usize);
    // Two seconds of 48 kHz audio, less the few samples the filter still holds back.
    assert!(
        session.window_16k.len().abs_diff(32_000) < 1_200,
        "got {}",
        session.window_16k.len()
    );
}

#[test]
fn advancing_the_window_drops_the_matching_16k_audio() {
    let mut session = StreamingSession::new();
    let rate = 48_000;
    session.ingest(&vec![0.1; 4 * rate as usize], rate).unwrap();
    let before = session.window_16k.len();

    session.advance_window(rate as usize, rate);

    assert_eq!(session.window_start, rate as usize);
    assert_eq!(session.window_16k.len(), before - 16_000);
}

#[test]
fn a_failed_conversion_restarts_from_the_window_start() {
    let mut session = StreamingSession::new();
    let rate = 16_000;
    session.ingest(&vec![0.1; 3 * rate as usize], rate).unwrap();
    session.advance_window(rate as usize, rate);

    session.restart_conversion();

    assert_eq!(session.fed(), rate as usize);
    assert_eq!(session.window_16k.len(), 0);
    assert!(session.converter.is_none());
}

#[test]
fn incremental_earshot_matches_a_fresh_scan_at_every_speech_onset() {
    let mut delayed = vec![0.0; 31 * VAD_CHUNK_16K + 19];
    delayed.extend(voiced(8_000));
    let quiet: Vec<f32> = voiced(8_000).iter().map(|s| s * 0.03).collect();
    for samples in [voiced(8_000), delayed, quiet, vec![0.0; 16_000]] {
        let expected = lead_speech_offset(&samples);
        for piece in [1, 7, 255, 256, 257, 3_200, 16_000] {
            let mut lead = LeadingSpeech::default();
            let mut end = 0;
            while end < samples.len() {
                end = (end + piece).min(samples.len());
                let found = lead.scan(&samples[..end]);
                assert!(lead.scanned <= end);
                if found.is_some() {
                    assert_eq!(found, expected, "piece size {piece}");
                    // Finding speech releases the detector and keeps the offset stable.
                    assert!(lead.detector.is_none());
                    assert_eq!(lead.scan(&samples), expected);
                    break;
                }
                assert_eq!(lead.scanned, end / VAD_CHUNK_16K * VAD_CHUNK_16K);
            }
            assert_eq!(lead.scan(&samples), expected, "piece size {piece}");
        }
    }
}

#[test]
fn conversion_restart_resets_the_incremental_vad_timeline() {
    let mut session = StreamingSession::new();
    session.ingest(&vec![0.0; 16_000], 16_000).unwrap();
    assert_eq!(session.leading_speech.scan(&session.window_16k), None);
    assert!(session.leading_speech.scanned > 0);

    session.restart_conversion();
    assert_eq!(session.leading_speech.scanned, 0);
    assert!(session.leading_speech.detector.is_none());
    let speech = voiced(8_000);
    session.ingest(&speech, 16_000).unwrap();
    assert_eq!(
        session.leading_speech.scan(&session.window_16k),
        lead_speech_offset(&speech)
    );
}

// ── silence ────────────────────────────────────────────────────────────

#[test]
fn quiet_speech_is_found_on_the_levelled_audio() {
    let quiet: Vec<f32> = voiced(2 * 16_000).iter().map(|s| s * 0.03).collect();
    assert_eq!(lead_speech_offset(&quiet), None, "raw audio should miss it");
    assert!(final_speech_offset(&quiet).is_some());
}

#[test]
fn a_silent_recording_has_no_speech_to_decode() {
    assert_eq!(final_speech_offset(&vec![0.0; 3 * 16_000]), None);
}
