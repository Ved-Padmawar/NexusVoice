//! Formatter API keys in the OS credential store: Credential Manager on
//! Windows, the Secret Service on Linux, the Keychain on macOS.

use std::sync::Arc;

use keyring_core::{CredentialStore, Entry, Error};

pub struct Secrets {
    /// The app identifier, so dev and release builds keep separate keys.
    service: String,
    store: Option<Arc<CredentialStore>>,
}

impl Secrets {
    /// Open the platform credential store. Without one (no Secret Service
    /// running, say) keys read as absent and saving fails.
    pub fn new(service: impl Into<String>) -> Self {
        let store = off_runtime(platform_store)
            .map_err(|e| log::warn!("no credential store available: {e}"))
            .ok();
        Self {
            service: service.into(),
            store,
        }
    }

    #[cfg(test)]
    pub fn mock() -> Self {
        Self {
            service: "nexusvoice-tests".into(),
            store: Some(keyring_core::mock::Store::new().unwrap()),
        }
    }

    fn entry(&self, provider: &str) -> Result<Entry, Error> {
        self.store.as_ref().ok_or(Error::NoDefaultStore)?.build(
            &self.service,
            &format!("formatter-api-key:{provider}"),
            None,
        )
    }

    /// `None` when no key is saved or the store cannot be read.
    pub fn api_key(&self, provider: &str) -> Option<String> {
        self.read_api_key(provider)
            .map_err(|e| log::warn!("{e}"))
            .ok()
            .flatten()
    }

    pub fn read_api_key(&self, provider: &str) -> Result<Option<String>, String> {
        match off_runtime(|| self.entry(provider)?.get_password()) {
            Ok(key) => Ok(Some(key)),
            Err(Error::NoEntry) => Ok(None),
            Err(e) => Err(format!("could not read the {provider} API key: {e}")),
        }
    }

    pub fn replace_api_key(&self, provider: &str, key: Option<&str>) -> Result<(), String> {
        match key {
            Some(key) => self.set_api_key(provider, key),
            None => self.delete_api_key(provider),
        }
    }

    pub fn set_api_key(&self, provider: &str, key: &str) -> Result<(), String> {
        off_runtime(|| self.entry(provider)?.set_password(key))
            .map_err(|e| format!("could not save the API key: {e}"))
    }

    fn delete_api_key(&self, provider: &str) -> Result<(), String> {
        match off_runtime(|| self.entry(provider)?.delete_credential()) {
            Ok(()) | Err(Error::NoEntry) => Ok(()),
            Err(e) => Err(format!("could not remove the API key: {e}")),
        }
    }
}

/// The Secret Service store drives zbus on its own tokio runtime, which
/// panics if entered from a task on another. `block_in_place` hands this
/// worker's tasks off first, so a call is safe from any thread.
fn off_runtime<T>(call: impl FnOnce() -> T) -> T {
    match tokio::runtime::Handle::try_current() {
        Ok(rt) if rt.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread => {
            tokio::task::block_in_place(call)
        }
        _ => call(),
    }
}

#[cfg(windows)]
fn platform_store() -> keyring_core::Result<Arc<CredentialStore>> {
    Ok(windows_native_keyring_store::Store::new()?)
}

#[cfg(target_os = "linux")]
fn platform_store() -> keyring_core::Result<Arc<CredentialStore>> {
    Ok(zbus_secret_service_keyring_store::Store::new()?)
}

#[cfg(target_os = "macos")]
fn platform_store() -> keyring_core::Result<Arc<CredentialStore>> {
    Ok(apple_native_keyring_store::keychain::Store::new()?)
}
