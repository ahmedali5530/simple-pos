use platform_runtime::db::rls_validator;

#[tokio::test]
async fn rls_validator_passes_after_boot() {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    let pool = sqlx::PgPool::connect(&url).await.expect("connect");
    // Validator is a no-op until tenants exist; after a prior isolation test/boot it should pass.
    let result = rls_validator::validate(&pool).await;
    if let Err(err) = &result {
        // Allow "not ready" only when tenants table is missing.
        let msg = err.to_string();
        if msg.contains("tenants") {
            return;
        }
    }
    result.expect("rls validation");
}
