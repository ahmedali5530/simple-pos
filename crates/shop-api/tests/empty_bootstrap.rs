//! Prove bootstrap from a genuinely empty database: migrate → GET /api/bootstrap.
//! Requires DATABASE_URL with rights to CREATE/DROP databases.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

fn admin_url() -> String {
    std::env::var("DATABASE_ADMIN_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .expect("DATABASE_URL is required for tests")
}

fn with_database_name(url: &str, name: &str) -> String {
    let (base, query) = match url.split_once('?') {
        Some((base, query)) => (base, Some(query)),
        None => (url, None),
    };
    let slash = base.rfind('/').expect("database URL needs a path");
    let mut next = format!("{}{name}", &base[..=slash]);
    if let Some(query) = query {
        next.push('?');
        next.push_str(query);
    }
    next
}

async fn create_empty_database() -> (String, String) {
    let admin = admin_url();
    let name = format!(
        "chaska_empty_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );
    let pool = sqlx::PgPool::connect(&admin)
        .await
        .expect("connect admin database");
    sqlx::query(&format!("CREATE DATABASE \"{name}\""))
        .execute(&pool)
        .await
        .expect("create empty database");
    pool.close().await;
    let url = with_database_name(&admin, &name);
    (name, url)
}

async fn drop_database(name: &str) {
    let admin = admin_url();
    let Ok(pool) = sqlx::PgPool::connect(&admin).await else {
        return;
    };
    let _ = sqlx::query(
        r#"
        SELECT pg_terminate_backend(pid)
        FROM pg_stat_activity
        WHERE datname = $1 AND pid <> pg_backend_pid()
        "#,
    )
    .bind(name)
    .execute(&pool)
    .await;
    let _ = sqlx::query(&format!("DROP DATABASE IF EXISTS \"{name}\""))
        .execute(&pool)
        .await;
    pool.close().await;
}

#[tokio::test]
async fn bootstrap_from_empty_database() {
    let (name, url) = create_empty_database().await;
    let outcome = run_bootstrap(&url).await;
    drop_database(&name).await;
    outcome.expect("bootstrap from empty database");
}

async fn run_bootstrap(url: &str) -> Result<(), String> {
    let app = shop_api::build_app(url)
        .await
        .map_err(|err| format!("migrate and build app on empty database: {err}"))?;
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/bootstrap")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .map_err(|err| format!("bootstrap request failed: {err}"))?;
    if response.status() != StatusCode::OK {
        return Err(format!("bootstrap status {}", response.status()));
    }
    let bytes = response
        .into_body()
        .collect()
        .await
        .map_err(|err| format!("bootstrap body: {err}"))?
        .to_bytes();
    let body: Value =
        serde_json::from_slice(&bytes).map_err(|err| format!("bootstrap json: {err}"))?;
    if body["needs_user"].as_bool() != Some(true) {
        return Err(format!("empty database must need the first user: {body}"));
    }
    let shop_name = body["shop_name"].as_str().unwrap_or("");
    if shop_name.is_empty() {
        return Err(format!("bootstrap must return a shop name: {body}"));
    }
    Ok(())
}
