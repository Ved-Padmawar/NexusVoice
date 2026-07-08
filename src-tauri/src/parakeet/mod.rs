//! Safe owner around parakeet.cpp's persistent C ABI.

use std::ffi::{c_char, c_float, c_int, c_void, CStr, CString};
use std::path::{Path, PathBuf};

use libloading::Library;

type Context = c_void;
type AbiVersion = unsafe extern "C" fn() -> c_int;
type Load = unsafe extern "C" fn(*const c_char) -> *mut Context;
type Free = unsafe extern "C" fn(*mut Context);
type TranscribePcm = unsafe extern "C" fn(
    *mut Context,
    *const c_float,
    c_int,
    c_int,
    c_int,
    *const c_char,
) -> *mut c_char;
type FreeString = unsafe extern "C" fn(*mut c_char);
type LastError = unsafe extern "C" fn(*mut Context) -> *const c_char;

pub struct ParakeetEngine {
    // The library must outlive all function pointers and the native context.
    _library: Library,
    context: *mut Context,
    free: Free,
    transcribe_pcm: TranscribePcm,
    free_string: FreeString,
    last_error: LastError,
}

// Calls are serialized by the application's Mutex. parakeet.cpp owns no Rust
// references and the context is destroyed before its dynamic library unloads.
unsafe impl Send for ParakeetEngine {}

impl ParakeetEngine {
    pub fn load(model_path: &Path, resource_dir: &Path) -> Result<Self, String> {
        let library_path = find_library(resource_dir)?;
        let model = CString::new(model_path.to_string_lossy().as_bytes())
            .map_err(|_| "model path contains a NUL byte".to_string())?;

        // SAFETY: symbols and signatures are from pinned parakeet_capi.h. The
        // Library is retained in Self for longer than every copied symbol.
        unsafe {
            let library = Library::new(&library_path)
                .map_err(|e| format!("load {}: {e}", library_path.display()))?;
            let abi: AbiVersion = *library
                .get(b"parakeet_capi_abi_version\0")
                .map_err(|e| format!("parakeet.cpp ABI symbol missing: {e}"))?;
            let version = abi();
            if version < 5 {
                return Err(format!(
                    "parakeet.cpp ABI {version} is too old; ABI 5+ is required"
                ));
            }
            let load: Load = *library.get(b"parakeet_capi_load\0").map_err(symbol_error)?;
            let free: Free = *library.get(b"parakeet_capi_free\0").map_err(symbol_error)?;
            let transcribe_pcm: TranscribePcm = *library
                .get(b"parakeet_capi_transcribe_pcm_lang\0")
                .map_err(symbol_error)?;
            let free_string: FreeString = *library
                .get(b"parakeet_capi_free_string\0")
                .map_err(symbol_error)?;
            let last_error: LastError = *library
                .get(b"parakeet_capi_last_error\0")
                .map_err(symbol_error)?;
            let context = load(model.as_ptr());
            if context.is_null() {
                return Err(format!(
                    "parakeet.cpp could not load {}",
                    model_path.display()
                ));
            }
            log::info!(
                "loaded {} with parakeet.cpp ABI {version}",
                model_path.display()
            );
            Ok(Self {
                _library: library,
                context,
                free,
                transcribe_pcm,
                free_string,
                last_error,
            })
        }
    }

    pub fn transcribe(&mut self, samples_16k: &[f32]) -> Result<String, String> {
        let count = c_int::try_from(samples_16k.len())
            .map_err(|_| "audio input exceeds parakeet.cpp limits".to_string())?;
        let language = CString::new("auto").expect("static string has no NUL");
        // SAFETY: context is live; samples remains valid for the call; the
        // returned allocation is copied before being freed by its owning ABI.
        unsafe {
            let output = (self.transcribe_pcm)(
                self.context,
                samples_16k.as_ptr(),
                count,
                16_000,
                0,
                language.as_ptr(),
            );
            if output.is_null() {
                return Err(self.error());
            }
            let text = CStr::from_ptr(output).to_string_lossy().trim().to_string();
            (self.free_string)(output);
            Ok(text)
        }
    }

    unsafe fn error(&self) -> String {
        let message = (self.last_error)(self.context);
        if message.is_null() {
            "parakeet.cpp transcription failed".into()
        } else {
            CStr::from_ptr(message).to_string_lossy().into_owned()
        }
    }
}

impl Drop for ParakeetEngine {
    fn drop(&mut self) {
        // SAFETY: this context was returned by this library's load function and
        // is freed exactly once before the Library field is dropped.
        unsafe { (self.free)(self.context) }
    }
}

// Passed by value so it can be used directly as `map_err(symbol_error)`; the
// owned error is consumed into the formatted string.
#[allow(clippy::needless_pass_by_value)]
fn symbol_error(error: libloading::Error) -> String {
    format!("parakeet.cpp ABI symbol missing: {error}")
}

fn find_library(resource_dir: &Path) -> Result<PathBuf, String> {
    let filename = if cfg!(target_os = "windows") {
        "parakeet.dll"
    } else if cfg!(target_os = "macos") {
        "libparakeet.dylib"
    } else {
        "libparakeet.so"
    };
    let mut candidates = Vec::new();
    if let Some(path) = std::env::var_os("NEXUSVOICE_PARAKEET_LIB") {
        candidates.push(PathBuf::from(path));
    }
    if let Some(path) = option_env!("NEXUSVOICE_BUNDLED_PARAKEET") {
        candidates.push(PathBuf::from(path));
    }
    candidates.push(resource_dir.join(filename));
    if let Ok(executable) = std::env::current_exe() {
        if let Some(parent) = executable.parent() {
            candidates.push(parent.join(filename));
        }
    }
    candidates
        .into_iter()
        .find(|path| path.is_file())
        .ok_or_else(|| format!("{filename} is missing from the application resources"))
}
