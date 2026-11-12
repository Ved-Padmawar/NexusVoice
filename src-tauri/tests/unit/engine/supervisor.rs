use super::*;

#[test]
fn deadlines_scale_with_the_audio_above_a_floor() {
    let gpu = Deadlines::for_device(true);
    assert_eq!(gpu.audio(0), gpu.call_floor);
    assert_eq!(gpu.audio(16_000 / 2), gpu.call_floor);
    assert_eq!(gpu.audio(16_000 * 5), Duration::from_secs(50));
    assert_eq!(gpu.run(16_000), gpu.run_floor);
    assert_eq!(gpu.run(16_000 * 10), Duration::from_secs(100));

    let cpu = Deadlines::for_device(false);
    assert_eq!(cpu.audio(16_000 * 10), Duration::from_secs(200));
    assert_eq!(cpu.run(16_000 * 60), Duration::from_mins(20));

    assert_eq!(ms_to_samples(1500), 24_000);
    assert_eq!(ms_to_samples(-5), 0);
}

#[test]
fn a_cpu_deadline_is_never_stricter_than_a_gpu_one() {
    let (gpu, cpu) = (Deadlines::for_device(true), Deadlines::for_device(false));
    for secs in [0, 1, 5, 30, 120, 600] {
        assert!(cpu.audio(secs * 16_000) >= gpu.audio(secs * 16_000));
        assert!(cpu.run(secs * 16_000) >= gpu.run(secs * 16_000));
    }
}
