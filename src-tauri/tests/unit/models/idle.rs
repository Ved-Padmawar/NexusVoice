use std::time::Duration;

use super::ModelUnload;

const MINUTE: Duration = Duration::from_secs(60);

#[test]
fn never_keeps_the_model_loaded() {
    assert!(!ModelUnload::Never.should_unload(24 * 60 * MINUTE, false));
}

#[test]
fn a_model_unloads_once_idle_past_the_limit() {
    let policy = ModelUnload::After5Minutes;
    assert!(!policy.should_unload(4 * MINUTE, false));
    assert!(policy.should_unload(5 * MINUTE, false));
    assert!(ModelUnload::After2Minutes.should_unload(2 * MINUTE, false));
    assert!(!ModelUnload::After10Minutes.should_unload(9 * MINUTE, false));
    assert!(ModelUnload::After1Hour.should_unload(60 * MINUTE, false));
}

#[test]
fn a_model_never_unloads_mid_recording() {
    assert!(!ModelUnload::After5Minutes.should_unload(60 * MINUTE, true));
}

#[test]
fn the_setting_round_trips_as_its_wire_name() {
    let json = serde_json::to_string(&ModelUnload::After15Minutes).unwrap();
    assert_eq!(json, "\"after15Minutes\"");
    assert_eq!(
        serde_json::from_str::<ModelUnload>(&json).unwrap(),
        ModelUnload::After15Minutes
    );
}
