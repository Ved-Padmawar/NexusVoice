use super::remove_fillers;

#[test]
fn universal_fillers_go_in_any_language() {
    assert_eq!(
        remove_fillers("so uh I think hmm yes", None),
        "so I think yes"
    );
    assert_eq!(remove_fillers("ja uhm genau", Some("de")), "ja genau");
    assert_eq!(remove_fillers("wait umm mmm ok", None), "wait ok");
}

#[test]
fn english_fillers_go_only_for_english() {
    assert_eq!(remove_fillers("so um I er think", Some("en")), "so I think");
    assert_eq!(
        remove_fillers("so um I er think", Some("en-GB")),
        "so I think"
    );
    // Portuguese "um" is "a"; German "er" is "he".
    assert_eq!(
        remove_fillers("comprei um carro", Some("pt")),
        "comprei um carro"
    );
    assert_eq!(remove_fillers("er kommt", None), "er kommt");
}

#[test]
fn real_words_that_resemble_fillers_survive() {
    assert_eq!(
        remove_fillers("to err is human", Some("en")),
        "to err is human"
    );
    assert_eq!(remove_fillers("cut 5 mm off", Some("en")), "cut 5 mm off");
    assert_eq!(remove_fillers("the hummus", Some("en")), "the hummus");
}

#[test]
fn a_leading_capitalized_filler_passes_its_capital_on() {
    assert_eq!(remove_fillers("Uh, so it works.", None), "So it works.");
    assert_eq!(
        remove_fillers("Done. Um, next one.", Some("en")),
        "Done. Next one."
    );
}

#[test]
fn a_sentence_end_on_the_filler_moves_to_the_previous_word() {
    assert_eq!(remove_fillers("it works, uh.", None), "it works.");
    assert_eq!(remove_fillers("really? hmm?", None), "really?");
    assert_eq!(remove_fillers("well uh... yes", None), "well... yes");
}

#[test]
fn a_transcript_of_only_fillers_becomes_empty() {
    assert_eq!(remove_fillers("Uh, umm... hmm.", None), "");
}

#[test]
fn spacing_and_line_breaks_are_kept() {
    let text = "first line\nsecond  line\n\n- a bullet";
    assert_eq!(remove_fillers(text, Some("en")), text);
    assert_eq!(remove_fillers("one\nuh two", None), "one\ntwo");
    assert_eq!(remove_fillers("one uh\ntwo", None), "one\ntwo");
    // A blank line between paragraphs survives either side of the filler.
    assert_eq!(remove_fillers("one\nuh\n\ntwo", None), "one\n\ntwo");
    assert_eq!(remove_fillers("one\n\nuh\ntwo", None), "one\n\ntwo");
}
