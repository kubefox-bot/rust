pub mod fixtures;
use axum::{body::Body, http::Request};
use hmail_cleaner_center::{
    config::BrowserOrigin,
    http::{router, AppState},
};
use sqlx::{sqlite::SqliteConnectOptions, Connection};
use tower::ServiceExt;
#[tokio::test]
async fn login_http_accepts_unicode_and_rejects_excess_scalars_before_verification() {
    let f = fixtures::center().await;
    let mut database = sqlx::SqliteConnection::connect_with(
        &SqliteConnectOptions::new().filename(f.root.path().join("center.sqlite")),
    )
    .await
    .unwrap();
    let state = AppState::new(
        f.store.clone(),
        BrowserOrigin::parse("https://localhost:8443").unwrap(),
    )
    .await
    .unwrap();
    let app = router(state);
    for (login, expected) in [
        ("я".repeat(40), 200),
        ("𐐀".repeat(64), 200),
        ("я".repeat(65), 400),
        ("𐐀".repeat(65), 400),
    ] {
        sqlx::query("UPDATE account SET login=? WHERE singleton=1")
            .bind(&login)
            .execute(&mut database)
            .await
            .unwrap();
        let response = app
            .clone()
            .oneshot(
                Request::post("/api/v1/auth/login")
                    .header("host", "localhost:8443")
                    .header("x-hmail-request", "1")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({"login":login,"password":"fixture-center-password"})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status().as_u16(), expected, "{login}");
    }
}
