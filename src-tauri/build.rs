fn main() {
    // Delay-load CUDA DLLs on Windows so the app starts on CPU-only machines.
    // Without this, Windows refuses to launch the .exe if nvcuda.dll is missing.
    #[cfg(all(target_os = "windows", feature = "cuda"))]
    {
        println!("cargo:rustc-link-arg=/DELAYLOAD:nvcuda.dll");
        println!("cargo:rustc-link-arg=/DELAYLOAD:cublas64_12.dll");
        println!("cargo:rustc-link-arg=/DELAYLOAD:cudart64_12.dll");
        println!("cargo:rustc-link-arg=delayimp.lib");
    }

    tauri_build::build()
}
