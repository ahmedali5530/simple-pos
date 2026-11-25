//! HTTP characterization tests for every legacy route.
//! Requires DATABASE_URL pointing at a Postgres that can create temporary databases.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

async fn app() -> axum::Router {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL is required for tests");
    shop_api::build_app(&url).await.expect("build app")
}

async fn json_body(response: axum::response::Response) -> Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap_or(Value::Null)
}

async fn get(app: &axum::Router, path: &str, token: Option<&str>) -> (StatusCode, Value) {
    let mut builder = Request::builder().method("GET").uri(path);
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    let response = app
        .clone()
        .oneshot(builder.body(Body::empty()).unwrap())
        .await
        .unwrap();
    (response.status(), json_body(response).await)
}

async fn post_json(
    app: &axum::Router,
    path: &str,
    body: Value,
    token: Option<&str>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", "application/json");
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    let response = app
        .clone()
        .oneshot(builder.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    (response.status(), json_body(response).await)
}

async fn ensure_manager(app: &axum::Router) -> String {
    let (status, bootstrap) = get(app, "/api/bootstrap", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(bootstrap.get("shop_name").is_some());

    let name = "Char Manager";
    let password = "test1234";

    if bootstrap["needs_user"].as_bool() == Some(true) {
        let (status, body) = post_json(
            app,
            "/api/users",
            json!({
                "name": name,
                "password": password,
                "role": "manager",
                "active": true
            }),
            None,
        )
        .await;
        assert!(
            status.is_success(),
            "create first user failed: {status} {body}"
        );
    } else {
        // Seed a known manager through SQL when the open bootstrap path is closed.
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
        let pool = sqlx::PgPool::connect(&url).await.expect("pool");
        let hash = bcrypt::hash(password, 10).expect("hash");
        sqlx::query(
            r#"
            INSERT INTO users (name, password_hash, role, active)
            VALUES ($1, $2, 'manager', TRUE)
            ON CONFLICT DO NOTHING
            "#,
        )
        .bind(name)
        .bind(&hash)
        .execute(&pool)
        .await
        .ok();
        // Unique index is on lower(name); update password if the row already exists.
        sqlx::query(
            r#"
            UPDATE users
            SET password_hash = $2, role = 'manager', active = TRUE
            WHERE lower(name) = lower($1)
            "#,
        )
        .bind(name)
        .bind(&hash)
        .execute(&pool)
        .await
        .expect("update test manager");
    }

    let (status, session) = post_json(
        app,
        "/api/login",
        json!({ "name": name, "password": password }),
        None,
    )
    .await;
    assert!(status.is_success(), "login: {status} {session}");
    session["token"].as_str().expect("token").to_string()
}

#[tokio::test]
async fn bootstrap_is_public() {
    let app = app().await;
    let (status, body) = get(&app, "/api/bootstrap", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.get("needs_user").is_some());
    assert!(body.get("shop_name").is_some());
}

#[tokio::test]
async fn protected_routes_require_auth() {
    let app = app().await;
    for path in [
        "/api/v1/pos/snapshot",
        "/api/v1/cash/desk",
        "/api/v1/reports/summary",
        "/api/v1/inventory/summary",
        "/api/v1/inventory/movements",
        "/api/session",
    ] {
        let (status, _) = get(&app, path, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{path}");
    }
}

#[tokio::test]
async fn pos_snapshot_shape() {
    let app = app().await;
    let token = ensure_manager(&app).await;
    let (status, body) = get(&app, "/api/v1/pos/snapshot", Some(&token)).await;
    assert_eq!(status, StatusCode::OK);
    for key in [
        "shop_name",
        "bill_prefix",
        "currency",
        "locale",
        "barcode_rules",
        "sections",
        "all_sections",
        "categories",
        "last_section_id",
    ] {
        assert!(body.get(key).is_some(), "missing {key}");
    }
    assert!(body["sections"].is_array());
    assert!(body["categories"].is_array());
}

#[tokio::test]
async fn legacy_bill_route_is_not_on_the_shop_router() {
    let app = app().await;
    let token = ensure_manager(&app).await;
    let (status, _) = post_json(&app, "/api/bills", serde_json::json!({}), Some(&token)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn session_and_logout() {
    let app = app().await;
    let token = ensure_manager(&app).await;
    let (status, user) = get(&app, "/api/session", Some(&token)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(user.get("id").is_some());
    assert!(user.get("name").is_some());
    assert!(user.get("role").is_some());

    let (status, _) = post_json(&app, "/api/logout", json!({}), Some(&token)).await;
    assert!(status == StatusCode::NO_CONTENT || status.is_success());
}

#[tokio::test]
async fn cash_desk_shape() {
    let app = app().await;
    let token = ensure_manager(&app).await;
    let (status, body) = get(&app, "/api/v1/cash/desk", Some(&token)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.get("suggested_opening").is_some());
    assert!(body.get("history").is_some());
}

#[tokio::test]
async fn reports_shape() {
    let app = app().await;
    let token = ensure_manager(&app).await;
    let (status, body) = get(&app, "/api/v1/reports/summary", Some(&token)).await;
    assert_eq!(status, StatusCode::OK);
    for key in [
        "sections",
        "bills",
        "movements",
        "bill_count",
        "total",
        "stock_units",
        "stock_value",
        "profit",
        "mix",
        "range_label",
    ] {
        assert!(body.get(key).is_some(), "missing {key}");
    }
}

#[tokio::test]
async fn stock_summary_and_movements() {
    let app = app().await;
    let token = ensure_manager(&app).await;
    let (status, body) = get(&app, "/api/v1/inventory/summary", Some(&token)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.is_array());
    let (status, body) = get(&app, "/api/v1/inventory/movements", Some(&token)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.is_array());
}

#[tokio::test]
async fn till_scan_settle_stock_cash_and_reports() {
    let app = app().await;
    let token = ensure_manager(&app).await;
    let (status, pos) = get(&app, "/api/v1/pos/snapshot", Some(&token)).await;
    assert_eq!(status, StatusCode::OK);

    let section_id = pos["sections"]
        .as_array()
        .and_then(|rows| rows.first())
        .and_then(|row| row["id"].as_i64());
    let item = pos["sections"]
        .as_array()
        .and_then(|rows| rows.first())
        .and_then(|row| row["items"].as_array())
        .and_then(|items| items.first().cloned());

    if let (Some(section_id), Some(item)) = (section_id, item) {
        let item_id = item["id"].as_i64().expect("item id");
        let rate = item["rate"].as_i64().unwrap_or(0);
        let (status, stock) = post_json(
            &app,
            "/api/v1/inventory/adjust",
            json!({
                "item_id": item_id,
                "qty": 1,
                "direction": "in",
                "note": "characterization"
            }),
            Some(&token),
        )
        .await;
        assert!(status.is_success(), "stock adjust: {status} {stock}");

        let (status, bill) = post_json(
            &app,
            "/api/v1/pos/bills",
            json!({
                "section_id": section_id,
                "lines": [{ "item_id": item_id, "rate": rate, "qty": 1 }]
            }),
            Some(&token),
        )
        .await;
        assert!(status.is_success(), "settle: {status} {bill}");
        assert!(bill.get("bill_no").is_some());
    }

    let (status, desk) = get(&app, "/api/v1/cash/desk", Some(&token)).await;
    assert_eq!(status, StatusCode::OK);
    if desk.get("open").is_none() || desk["open"].is_null() {
        let (status, opened) = post_json(
            &app,
            "/api/v1/cash/open",
            json!({ "opening_cash": 0, "petty_cash": 0 }),
            Some(&token),
        )
        .await;
        assert!(status.is_success(), "open cash: {status} {opened}");
    }

    let (status, closed) = post_json(
        &app,
        "/api/v1/cash/close",
        json!({ "expenses": [], "cash_taken": 0, "manual_count": 0 }),
        Some(&token),
    )
    .await;
    assert!(status.is_success(), "close cash: {status} {closed}");

    let (status, reports) = get(&app, "/api/v1/reports/summary", Some(&token)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(reports.get("bill_count").is_some());
}

#[tokio::test]
async fn upgrade_from_legacy_migrations() {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    let pool = shop_api::connect(&url)
        .await
        .expect("connect runs frozen migrations 001-016");
    let version: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(version), 0) FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .expect("read migration version");
    assert!(
        version >= 16,
        "expected migrations through 016, got {version}"
    );
    let tables: Vec<(String,)> = sqlx::query_as(
        r#"
        SELECT table_name
        FROM information_schema.tables
        WHERE table_schema = 'public'
          AND table_name = ANY($1)
        ORDER BY table_name
        "#,
    )
    .bind(&[
        "sections",
        "categories",
        "items",
        "item_variants",
        "bills",
        "bill_lines",
        "stock_movements",
        "pos_state",
        "users",
        "sessions",
        "shop_settings",
        "barcode_rules",
        "cash_shifts",
        "cash_expenses",
    ])
    .fetch_all(&pool)
    .await
    .expect("list tables");
    assert_eq!(tables.len(), 14, "legacy tables missing: {tables:?}");
}
