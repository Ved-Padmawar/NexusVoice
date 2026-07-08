use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    tauri_build::build();
    println!("cargo:rerun-if-env-changed=NEXUSVOICE_PARAKEET_BACKEND");
    println!("cargo:rerun-if-env-changed=NEXUSVOICE_SKIP_NATIVE_BUILD");
    println!("cargo:rerun-if-changed=../vendor/parakeet.cpp/CMakeLists.txt");

    if std::env::var_os("NEXUSVOICE_SKIP_NATIVE_BUILD").is_some() {
        return;
    }

    let backend = std::env::var("NEXUSVOICE_PARAKEET_BACKEND")
        .unwrap_or_else(|_| "cpu".to_string());
    assert!(matches!(backend.as_str(), "cpu" | "vulkan" | "cuda"));
    let profile = std::env::var("PROFILE").expect("Cargo PROFILE is set");
    let source = PathBuf::from("../vendor/parakeet.cpp");
    assert!(
        source.join("CMakeLists.txt").is_file(),
        "parakeet.cpp submodule is missing; run git submodule update --init --recursive"
    );
    let build = source.join(format!("build-nexusvoice-{profile}-{backend}"));

    let mut configure = Command::new("cmake");
    configure
        .arg("-S")
        .arg(&source)
        .arg("-B")
        .arg(&build)
        .arg("-DPARAKEET_SHARED=ON")
        .arg("-DPARAKEET_BUILD_CLI=OFF")
        .arg("-DPARAKEET_BUILD_SERVER=OFF")
        .arg("-DPARAKEET_BUILD_TESTS=OFF")
        .arg("-DBUILD_SHARED_LIBS=OFF")
        // ggml is linked statically into the shared parakeet library, so every
        // ggml object must be position-independent or the ELF link fails
        // (R_X86_64_PC32 against a static symbol). No-op on MSVC.
        .arg("-DCMAKE_POSITION_INDEPENDENT_CODE=ON")
        .arg("-DGGML_NATIVE=OFF")
        .arg(format!("-DPARAKEET_GGML_VULKAN={}", on(backend == "vulkan")))
        .arg(format!("-DPARAKEET_GGML_CUDA={}", on(backend == "cuda")));
    run(&mut configure, "configure parakeet.cpp");

    let mut compile = Command::new("cmake");
    compile
        .arg("--build")
        .arg(&build)
        .arg("--config")
        .arg("Release")
        .arg("--target")
        .arg("parakeet")
        .arg("--parallel");
    run(&mut compile, "build parakeet.cpp");

    let filename = native_filename();
    let library = find_file(&build, filename)
        .unwrap_or_else(|| panic!("CMake completed but {filename} was not produced"));
    println!("cargo:rustc-env=NEXUSVOICE_BUNDLED_PARAKEET={}", library.display());
}

fn on(enabled: bool) -> &'static str {
    if enabled { "ON" } else { "OFF" }
}

fn native_filename() -> &'static str {
    if cfg!(target_os = "windows") {
        "parakeet.dll"
    } else if cfg!(target_os = "macos") {
        "libparakeet.dylib"
    } else {
        "libparakeet.so"
    }
}

fn find_file(directory: &Path, filename: &str) -> Option<PathBuf> {
    std::fs::read_dir(directory).ok()?.filter_map(Result::ok).find_map(|entry| {
        let path = entry.path();
        if path.is_dir() {
            find_file(&path, filename)
        } else if path.file_name().is_some_and(|name| name == filename) {
            Some(path.canonicalize().unwrap_or(path))
        } else {
            None
        }
    })
}

fn run(command: &mut Command, description: &str) {
    let status = command.status().unwrap_or_else(|error| panic!("{description}: {error}"));
    assert!(status.success(), "failed to {description}");
}
