use super::error::ApiError;

pub fn open_logs_folder() -> Result<(), ApiError> {
    let logs_dir = crate::platform::paths::log_dir();
    std::fs::create_dir_all(&logs_dir).map_err(|e| ApiError::new("io_error", e.to_string()))?;
    opener::open(&logs_dir).map_err(|e| ApiError::new("open_error", e.to_string()))
}
