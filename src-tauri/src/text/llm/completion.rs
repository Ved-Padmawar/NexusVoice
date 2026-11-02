//! What the formatter transports hand back.

#[derive(Debug, PartialEq, Eq)]
pub struct Completion {
    pub text: String,
    /// The model stopped at its token limit, so `text` is cut short.
    pub truncated: bool,
}
