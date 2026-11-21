use std::sync::Arc;

use async_trait::async_trait;
use catalog_contract::{CatalogQuery, ItemRef, SectionRef};
use platform_kernel::capability::{Capability, CapabilityEffect, CapabilityHandler, CapabilityKind};
use platform_kernel::context::TenantTx;
use platform_kernel::contract::{DeletionVeto, Registrar};
use platform_kernel::error::AppError;
use platform_kernel::manifest::{NavItem, PluginManifest, SettingsPage};
use platform_kernel::migrate::Migration;
use platform_kernel::permission::{builtins, Permission};
use platform_kernel::plugin::Plugin;
use serde_json::json;

pub struct CatalogPlugin;

impl Plugin for CatalogPlugin {
    fn manifest(&self) -> PluginManifest {
        {
            let mut manifest = PluginManifest::v1("catalog", "Catalog", "0.1.0");
            manifest.description = "Sections, categories, items, variants and pictures.".into();
            manifest.permissions = vec![
                Permission::new(builtins::CATALOG_READ),
                Permission::new(builtins::CATALOG_WRITE),
            ];
            manifest.capabilities = vec![Capability {
                id: "catalog.items.list".into(),
                kind: CapabilityKind::Query,
                permission: builtins::CATALOG_READ.into(),
                effect: CapabilityEffect::Read,
                requires_confirmation: false,
                input_schema: json!({"type":"object","properties":{}}),
                output_schema: json!({"type":"object"}),
                description: "List active catalog items via POS snapshot sections.".into(),
            }];
            manifest.nav = vec![NavItem {
                id: "settings".into(),
                label: "Settings".into(),
                href: "/settings/items".into(),
                permission: Some(builtins::CATALOG_WRITE.into()),
                order: 50,
            }];
            manifest.settings_pages = vec![
                SettingsPage {
                    id: "items".into(),
                    label: "Items".into(),
                    href: "/settings/items".into(),
                    permission: Some(builtins::CATALOG_WRITE.into()),
                    order: 10,
                },
                SettingsPage {
                    id: "sections".into(),
                    label: "Sections".into(),
                    href: "/settings/sections".into(),
                    permission: Some(builtins::CATALOG_WRITE.into()),
                    order: 20,
                },
                SettingsPage {
                    id: "categories".into(),
                    label: "Categories".into(),
                    href: "/settings/categories".into(),
                    permission: Some(builtins::CATALOG_WRITE.into()),
                    order: 30,
                },
                SettingsPage {
                    id: "users".into(),
                    label: "Users".into(),
                    href: "/settings/users".into(),
                    permission: Some(builtins::USERS_WRITE.into()),
                    order: 40,
                },
            ];
            manifest
        }
    }

    fn migrations(&self) -> &'static [Migration] {
        &[]
    }

    fn register(&self, registrar: &mut Registrar) {
        registrar.provide(Arc::new(CatalogQueryService));
        registrar.deletion_veto(Arc::new(CatalogDeletionVeto));
        registrar.capability(
            Capability {
                id: "catalog.items.list".into(),
                kind: CapabilityKind::Query,
                permission: builtins::CATALOG_READ.into(),
                effect: CapabilityEffect::Read,
                requires_confirmation: false,
                input_schema: json!({"type":"object","properties":{}}),
                output_schema: json!({"type":"object"}),
                description: "List items".into(),
            },
            Arc::new(ListItemsCapability),
        );
        let _ = NavItem {
            id: String::new(),
            label: String::new(),
            href: String::new(),
            permission: None,
            order: 0,
        };
    }
}

struct CatalogQueryService;

#[async_trait]
impl CatalogQuery for CatalogQueryService {
    async fn get_item(&self, tx: &mut TenantTx<'_>, id: i32) -> Result<Option<ItemRef>, AppError> {
        let row: Option<(i32, String, i32, i32, Option<String>, i32, bool)> = sqlx::query_as(
            r#"
            SELECT id, name, section_id, category_id, barcode, rate, active
            FROM items WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(tx.conn())
        .await
        .map_err(AppError::from)?;
        Ok(row.map(|(id, name, section_id, category_id, barcode, rate, active)| ItemRef {
            id,
            name,
            section_id,
            category_id,
            barcode,
            rate,
            active,
        }))
    }

    async fn list_sections(&self, tx: &mut TenantTx<'_>) -> Result<Vec<SectionRef>, AppError> {
        let rows: Vec<(i32, String, String)> = sqlx::query_as(
            "SELECT id, name, short_name FROM sections ORDER BY pos_order, id",
        )
        .fetch_all(tx.conn())
        .await
        .map_err(AppError::from)?;
        Ok(rows
            .into_iter()
            .map(|(id, name, short_name)| SectionRef {
                id,
                name,
                short_name,
            })
            .collect())
    }
}

struct CatalogDeletionVeto;

#[async_trait]
impl DeletionVeto for CatalogDeletionVeto {
    async fn check(
        &self,
        tx: &mut TenantTx<'_>,
        entity: &str,
        id: i32,
    ) -> Result<Option<String>, AppError> {
        if entity == "section" {
            let items: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM items WHERE section_id = $1")
                .bind(id)
                .fetch_one(tx.conn())
                .await?;
            if items > 0 {
                return Ok(Some(
                    "Move this section's items before removing it.".into(),
                ));
            }
        }
        if entity == "category" {
            let items: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM items WHERE category_id = $1")
                    .bind(id)
                    .fetch_one(tx.conn())
                    .await?;
            if items > 0 {
                return Ok(Some(
                    "Move this category's items before removing it.".into(),
                ));
            }
        }
        Ok(None)
    }
}

struct ListItemsCapability;

#[async_trait]
impl CapabilityHandler for ListItemsCapability {
    async fn invoke(
        &self,
        tx: &mut TenantTx<'_>,
        _input: serde_json::Value,
    ) -> Result<serde_json::Value, AppError> {
        let rows: Vec<(i32, String, i32)> =
            sqlx::query_as("SELECT id, name, rate FROM items WHERE active ORDER BY sort_order, id")
                .fetch_all(tx.conn())
                .await?;
        Ok(json!({
            "items": rows.into_iter().map(|(id, name, rate)| json!({
                "id": id, "name": name, "rate": rate
            })).collect::<Vec<_>>()
        }))
    }
}
