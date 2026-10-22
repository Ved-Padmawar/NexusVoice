//! Models are gigabytes, so the dev build must share the release download dir
//! rather than refetching into its own.

use std::path::Path;

use super::models_dir_for;

#[test]
fn the_dev_build_shares_the_release_models_dir() {
    let dev = models_dir_for(Path::new("/data/com.nexusvoice.app.dev"));
    assert_eq!(dev, Path::new("/data/com.nexusvoice.app/models"));
}

#[test]
fn the_release_build_keeps_models_under_its_own_dir() {
    let release = models_dir_for(Path::new("/data/com.nexusvoice.app"));
    assert_eq!(release, Path::new("/data/com.nexusvoice.app/models"));
}
