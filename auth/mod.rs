pub mod errors;
pub mod service;
pub mod session;

pub use errors::AuthError;
pub use service::AuthService;

#[cfg(test)]
pub use session::SessionState;

#[cfg(test)]
mod tests {
  use sqlx::sqlite::SqlitePoolOptions;

  use crate::database::connection::init_database;
  use crate::database::repositories::user::UserRepository;

  use super::{AuthError, AuthService, SessionState};

  #[tokio::test]
  async fn register_and_login_flow() {
    let pool = SqlitePoolOptions::new()
      .max_connections(1)
      .connect("sqlite::memory:")
      .await
      .expect("pool");
    init_database(&pool).await.expect("migrations");

    let service = AuthService::new(pool);

    let user = service
      .register("person@example.com", "secret")
      .await
      .expect("register");

    assert_eq!(user.email, "person@example.com");

    let current = service.current_user().await.expect("session user");
    assert_eq!(current.id, user.id);

    service.logout().await;
    assert!(service.current_user().await.is_none());

    let logged_in = service
      .login("person@example.com", "secret")
      .await
      .expect("login");

    assert_eq!(logged_in.id, user.id);
  }

  #[tokio::test]
  async fn duplicate_email_rejected() {
    let pool = SqlitePoolOptions::new()
      .max_connections(1)
      .connect("sqlite::memory:")
      .await
      .expect("pool");
    init_database(&pool).await.expect("migrations");

    let service = AuthService::new(pool);

    service
      .register("dup@example.com", "secret")
      .await
      .expect("register");

    let err = service
      .register("dup@example.com", "secret")
      .await
      .expect_err("duplicate should fail");

    match err {
      AuthError::EmailTaken => {}
      other => panic!("unexpected error: {other:?}"),
    }
  }

  #[tokio::test]
  async fn login_invalid_password() {
    let pool = SqlitePoolOptions::new()
      .max_connections(1)
      .connect("sqlite::memory:")
      .await
      .expect("pool");
    init_database(&pool).await.expect("migrations");

    let service = AuthService::new(pool);

    service
      .register("invalid@example.com", "secret")
      .await
      .expect("register");

    let err = service
      .login("invalid@example.com", "wrong")
      .await
      .expect_err("login should fail");

    match err {
      AuthError::InvalidCredentials => {}
      other => panic!("unexpected error: {other:?}"),
    }
  }

  #[tokio::test]
  async fn dependency_injection() {
    let pool = SqlitePoolOptions::new()
      .max_connections(1)
      .connect("sqlite::memory:")
      .await
      .expect("pool");
    init_database(&pool).await.expect("migrations");

    let repo = UserRepository::new(pool);
    let session = SessionState::new();
    let service = AuthService::with_dependencies(repo, session);

    let user = service
      .register("di@example.com", "secret")
      .await
      .expect("register");

    assert_eq!(user.email, "di@example.com");
  }
}
