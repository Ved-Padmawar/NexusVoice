fn main() {
    // Each Linux bundler lands the resources somewhere different, so list every
    // layout. deb/rpm use /usr/lib/<productName> — capitalised; a case mismatch
    // is silent at build time and fatal at launch. Windows needs no rpath.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux") {
        const RPATH: &str =
            "$ORIGIN:$ORIGIN/../lib/NexusVoice:$ORIGIN/../lib/nexusvoice:$ORIGIN/../lib";
        println!("cargo:rustc-link-arg=-Wl,-rpath,{RPATH}");
        // Otherwise the rpath isn't inherited by the ggml modules transcribe dlopen's.
        println!("cargo:rustc-link-arg=-Wl,--disable-new-dtags");
    }

    stage_transcribe_runtime_libs();

    tauri_build::build();
}

/// Stage transcribe-cpp's shared libraries and ggml backend modules into
/// `transcribe-libs/` so the bundler ships them beside the executable. Without
/// them the app registers zero compute devices.
///
/// Adapted from Handy (github.com/cjpais/Handy, MIT).
fn stage_transcribe_runtime_libs() {
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::PathBuf;

    println!("cargo:rerun-if-env-changed=DEP_TRANSCRIBE_CPP_RUNTIME_DIR");
    println!("cargo:rerun-if-env-changed=DEP_TRANSCRIBE_CPP_MODULE_DIR");

    // Set only in a shared posture; a static build has nothing to ship.
    let Some(runtime_dir) = std::env::var_os("DEP_TRANSCRIBE_CPP_RUNTIME_DIR") else {
        return;
    };

    // Two directories: the shared libs, and the dlopen'd backend modules.
    // Often the same one. Both must sit next to the executable.
    let mut dirs = BTreeSet::new();
    dirs.insert(PathBuf::from(runtime_dir));
    if let Some(module_dir) = std::env::var_os("DEP_TRANSCRIBE_CPP_MODULE_DIR") {
        dirs.insert(PathBuf::from(module_dir));
    }

    let dest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("transcribe-libs");
    // Recreate clean so a dropped module cannot linger from an earlier build.
    let _ = std::fs::remove_dir_all(&dest);
    std::fs::create_dir_all(&dest).expect("create transcribe-libs staging dir");

    // Collect across both dirs first so pruning sees each lib's whole family.
    let mut libs: BTreeMap<String, PathBuf> = BTreeMap::new();
    for dir in &dirs {
        println!("cargo:rerun-if-changed={}", dir.display());
        for entry in std::fs::read_dir(dir)
            .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
            .flatten()
        {
            let src = entry.path();
            let name = src.file_name().and_then(|s| s.to_str()).unwrap_or("");
            // A versioned Linux name (libfoo.so.0.2.0) has no `.so` extension,
            // so the substring check catches what `extension()` misses.
            let by_ext = src
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("dll") || e.eq_ignore_ascii_case("so"));
            if by_ext || name.contains(".so.") {
                libs.insert(name.to_string(), src);
            }
        }
    }

    // On Linux each lib is a symlink chain and the deb/rpm bundlers flatten
    // symlinks into real files, so staging every name would duplicate each lib.
    // Keep one name per lib: the shortest versioned name is the SONAME, and a
    // dlopen'd module usually has only its bare name.
    let mut best: BTreeMap<&str, (&str, &PathBuf, usize)> = BTreeMap::new();
    for (name, src) in &libs {
        let (stem, rank) = match split_versioned_so(name) {
            // Windows names are unversioned; keep as-is.
            None => (name.as_str(), 0),
            Some((stem, 0)) => (stem, usize::MAX),
            Some((stem, depth)) => (stem, depth - 1),
        };
        match best.get(stem) {
            Some(&(_, _, existing)) if existing <= rank => {}
            _ => {
                best.insert(stem, (name, src, rank));
            }
        }
    }

    let mut copied = 0usize;
    for &(name, src, _) in best.values() {
        std::fs::copy(src, dest.join(name))
            .unwrap_or_else(|e| panic!("copy {}: {e}", src.display()));
        copied += 1;
    }
    assert!(
        copied > 0,
        "no transcribe-cpp runtime libraries found under {dirs:?}; without them \
         the app registers zero compute devices"
    );
    println!("cargo:warning=Staged {copied} transcribe-cpp runtime library file(s)");
}

/// Split a versioned Linux library name into its stem and version depth:
/// `libfoo.so.0.2.0` → `("libfoo.so", 3)`, `libfoo.so` → `("libfoo.so", 0)`.
/// Returns `None` for names that are not `.so` at all (Windows `.dll`).
fn split_versioned_so(name: &str) -> Option<(&str, usize)> {
    let idx = name.find(".so")?;
    let stem = &name[..idx + 3];
    let depth = name[idx + 3..]
        .split('.')
        .filter(|part| !part.is_empty())
        .count();
    Some((stem, depth))
}
