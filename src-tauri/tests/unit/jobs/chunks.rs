use super::*;

#[test]
fn cuts_at_the_pause_near_the_target() {
    let mut samples = vec![0.5; RATE as usize * 45];
    let pause = TARGET - RATE as usize;
    samples[pause..pause + BLOCK].fill(0.0);

    let ends = ends(&samples);

    assert_eq!(ends[0], pause);
    assert_eq!(ends.last(), Some(&samples.len()));
}

#[test]
fn a_short_tail_joins_the_last_chunk() {
    let samples = vec![0.5; TARGET + RATE as usize];
    assert_eq!(ends(&samples), vec![samples.len()]);
}
