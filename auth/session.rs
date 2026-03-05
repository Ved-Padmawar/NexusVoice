use std::sync::Arc;

use tokio::sync::Mutex;

use crate::database::models::user::User;

#[derive(Debug, Default, Clone)]
pub struct SessionState {
    current_user: Arc<Mutex<Option<User>>>,
}

impl SessionState {
    pub fn new() -> Self {
        Self {
            current_user: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn set_user(&self, user: User) {
        let mut guard = self.current_user.lock().await;
        *guard = Some(user);
    }

    pub async fn clear_user(&self) {
        let mut guard = self.current_user.lock().await;
        *guard = None;
    }

    #[cfg(test)]
    pub async fn get_user(&self) -> Option<User> {
        let guard = self.current_user.lock().await;
        guard.clone()
    }
}
