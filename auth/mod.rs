pub mod errors;
pub mod service;
pub mod session;
pub mod tokens;

pub use errors::AuthError;
pub use service::AuthService;
pub use tokens::TokenPair;

#[cfg(test)]
pub use session::SessionState;

#[cfg(test)]
mod tests {
    use sqlx::sqlite::SqlitePoolOptions;

    use crate::database::connection::init_database;
    use crate::database::repositories::token::TokenRepository;
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

        let users = UserRepository::new(pool.clone());
        let tokens = TokenRepository::new(pool);
        let session = SessionState::new();
        let service = AuthService::with_dependencies(users, tokens, session);

        let user = service
            .register("di@example.com", "secret")
            .await
            .expect("register");

        assert_eq!(user.email, "di@example.com");
    }

    async fn make_service() -> AuthService {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("pool");
        init_database(&pool).await.expect("migrations");
        AuthService::new(pool)
    }

    #[tokio::test]
    async fn login_with_tokens_returns_pair() {
        let svc = make_service().await;
        svc.register("tok@example.com", "pass123").await.expect("register");
        let (_user, pair) = svc
            .login_with_tokens("tok@example.com", "pass123")
            .await
            .expect("login_with_tokens");
        assert!(!pair.access_token.is_empty());
        assert_eq!(pair.refresh_token.len(), 64);
        assert!(pair.expires_in_seconds > 0);
    }

    #[tokio::test]
    async fn token_rotation_works() {
        let svc = make_service().await;
        svc.register("rot@example.com", "pass123").await.expect("register");
        let (_user, pair1) = svc
            .login_with_tokens("rot@example.com", "pass123")
            .await
            .expect("login");

        let pair2 = svc
            .refresh_tokens(&pair1.refresh_token)
            .await
            .expect("refresh");

        // Old token should now be revoked
        let err = svc
            .refresh_tokens(&pair1.refresh_token)
            .await
            .expect_err("should be revoked");
        assert!(matches!(err, AuthError::TokenRevoked));

        // New refresh token must be different (rotation)
        assert_ne!(pair1.refresh_token, pair2.refresh_token);
        // New pair must be usable
        assert!(!pair2.access_token.is_empty());
    }

    #[tokio::test]
    async fn revoke_token_invalidates_refresh() {
        let svc = make_service().await;
        svc.register("rev@example.com", "pass123").await.expect("register");
        let (_user, pair) = svc
            .login_with_tokens("rev@example.com", "pass123")
            .await
            .expect("login");

        svc.revoke_token(&pair.refresh_token).await.expect("revoke");

        let err = svc
            .refresh_tokens(&pair.refresh_token)
            .await
            .expect_err("should fail");
        assert!(matches!(err, AuthError::TokenRevoked));
    }

    #[tokio::test]
    async fn access_token_validates() {
        let svc = make_service().await;
        let user = svc.register("val@example.com", "pass123").await.expect("register");
        let (_u, pair) = svc
            .login_with_tokens("val@example.com", "pass123")
            .await
            .expect("login");

        let user_id = svc.validate_token(&pair.access_token).expect("validate");
        assert_eq!(user_id, user.id);
    }

    #[tokio::test]
    async fn invalid_access_token_rejected() {
        let svc = make_service().await;
        let err = svc.validate_token("not.a.real.token").expect_err("should fail");
        assert!(matches!(err, AuthError::TokenInvalid));
    }
}
