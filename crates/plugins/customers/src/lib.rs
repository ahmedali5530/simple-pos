use axum::Json;
use platform_kernel::error::AppError;
use platform_kernel::manifest::{NavItem, PluginManifest};
use platform_kernel::migrate::Migration;
use platform_kernel::permission::Permission;
use platform_kernel::plugin::Plugin;
use platform_kernel::routes::{PluginRoutes, RegistrationError, TenantDb};
use serde::Deserialize;
use serde_json::{json, Value};

pub struct CustomersPlugin;

impl Plugin for CustomersPlugin {
    fn manifest(&self) -> PluginManifest {
        let mut manifest = PluginManifest::v1("customers", "Customers", "0.1.0");
        manifest.description = "People a shop sells to.".into();
        manifest.permissions = vec![
            Permission::new("customers.read"),
            Permission::new("customers.write"),
        ];
        manifest.nav = vec![NavItem {
            id: "customers".into(),
            label: "Customers".into(),
            href: "/customers".into(),
            permission: Some("customers.read".into()),
            order: 45,
        }];
        manifest
    }

    fn migrations(&self) -> &'static [Migration] {
        static MIGRATIONS: &[Migration] = &[Migration::new(
            1,
            "customers",
            include_str!("../migrations/001_customers.sql"),
        )];
        MIGRATIONS
    }

    fn register(&self, _registrar: &mut platform_kernel::contract::Registrar) {}

    fn routes(&self, routes: &mut PluginRoutes) -> Result<(), RegistrationError> {
        routes.get("", list, "customers.read")?;
        routes.post("", create, "customers.write")?;
        Ok(())
    }
}

async fn list(db: TenantDb) -> Result<Json<Value>, AppError> {
    let mut tx = db.begin(true).await?;
    let rows: Vec<(i32, String, String)> =
        sqlx::query_as("SELECT id, name, phone FROM customers ORDER BY name")
            .fetch_all(tx.conn())
            .await?;
    tx.commit().await?;
    Ok(Json(json!(rows.into_iter().map(|(id, name, phone)| json!({ "id": id, "name": name, "phone": phone })).collect::<Vec<_>>())))
}

#[derive(Deserialize)]
struct NewCustomer {
    name: String,
    #[serde(default)]
    phone: String,
}

async fn create(db: TenantDb, Json(body): Json<NewCustomer>) -> Result<Json<Value>, AppError> {
    let mut tx = db.begin(false).await?;
    let id: i32 = sqlx::query_scalar(
        "INSERT INTO customers (name, phone) VALUES ($1, $2) RETURNING id",
    )
    .bind(body.name.trim())
    .bind(body.phone.trim())
    .fetch_one(tx.conn())
    .await?;
    tx.commit().await?;
    Ok(Json(json!({ "id": id })))
}
