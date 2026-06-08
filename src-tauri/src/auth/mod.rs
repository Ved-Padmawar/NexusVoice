pub mod errors;
pub mod service;

pub use errors::AuthError;
pub use service::AuthService;

#[cfg(test)]
mod tests {
    use sqlx::sqlite::SqlitePoolOptions;

    use crate::database::connection::init_database;
    use crate::database::repositories::session::SessionRepository;
    use crate::database::repositories::user::UserRepository;

    use super::{AuthError, AuthService};

    async fn make_service() -> (AuthService, sqlx::SqlitePool) {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("pool");
        init_database(&pool).await.expect("migrations");
        (AuthService::new(pool.clone()), pool)
    }

    #[tokio::test]
    async fn register_persists_session_and_login_restores_it() {
        let (service, _pool) = make_service().await;

        let user = service
            .register("person@example.com", "secret")
            .await
            .expect("register");
        assert_eq!(user.email, "person@example.com");

        // Registering signs the user in — current_user reflects the persisted row.
        let current = service.current_user().await.expect("query").expect("user");
        assert_eq!(current.id, user.id);

        // Logout clears the persisted session.
        service.logout().await.expect("logout");
        assert!(service.current_user().await.expect("query").is_none());

        // Login re-establishes it.
        let logged_in = service
            .login("person@example.com", "secret")
            .await
            .expect("login");
        assert_eq!(logged_in.id, user.id);
        assert_eq!(
            service.current_user().await.expect("query").expect("user").id,
            user.id
        );
    }

    #[tokio::test]
    async fn duplicate_email_rejected() {
        let (service, _pool) = make_service().await;

        service
            .register("dup@example.com", "secret")
            .await
            .expect("register");

        let err = service
            .register("dup@example.com", "secret")
            .await
            .expect_err("duplicate should fail");
        assert!(matches!(err, AuthError::EmailTaken));
    }

    #[tokio::test]
    async fn login_invalid_password() {
        let (service, _pool) = make_service().await;

        service
            .register("invalid@example.com", "secret")
            .await
            .expect("register");

        let err = service
            .login("invalid@example.com", "wrong")
            .await
            .expect_err("login should fail");
        assert!(matches!(err, AuthError::InvalidCredentials));
    }

    #[tokio::test]
    async fn login_unknown_email() {
        let (service, _pool) = make_service().await;
        let err = service
            .login("nobody@example.com", "secret")
            .await
            .expect_err("should fail");
        assert!(matches!(err, AuthError::InvalidCredentials));
    }

    #[tokio::test]
    async fn dependency_injection() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("pool");
        init_database(&pool).await.expect("migrations");

        let users = UserRepository::new(pool.clone());
        let session = SessionRepository::new(pool);
        let service = AuthService::with_dependencies(users, session);

        let user = service
            .register("di@example.com", "secret")
            .await
            .expect("register");
        assert_eq!(user.email, "di@example.com");
    }
}
