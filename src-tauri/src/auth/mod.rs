pub mod errors;
pub mod service;

pub use errors::AuthError;
pub use service::AuthService;

#[cfg(test)]
#[path = "../../tests/unit/auth/mod.rs"]
mod tests;
