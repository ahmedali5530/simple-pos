use async_trait::async_trait;
use cash_contract::{
    CashClosing, CashClosingProvider, CashClosingQuery, CashExpense as ContractCashExpense,
};
use platform_kernel::context::TenantTx;
use platform_kernel::contract::Registrar;
use platform_kernel::error::AppError;
use platform_kernel::manifest::{Dependency, NavItem, PluginManifest};
use platform_kernel::migrate::Migration;
use platform_kernel::permission::Permission;
use platform_kernel::plugin::Plugin;
use platform_kernel::routes::{PluginRoutes, RegistrationError};

pub mod desk;
mod routes;

pub struct CashPlugin;

/// Composition handle consumed by the generated `shop-api` plugin list.
pub fn plugin() -> std::sync::Arc<dyn Plugin> {
    std::sync::Arc::new(CashPlugin)
}

impl Plugin for CashPlugin {
    fn manifest(&self) -> PluginManifest {
        let mut manifest = PluginManifest::v1("cash", "Cash", "0.1.0");
        manifest.default_enabled = true;
        manifest.description = "Cash shifts and expenses.".into();
        // Sales totals come from Point of Sale when enabled.
        manifest.optional = vec![
            Dependency::new("pos", "^0.1"),
            Dependency::new("printing", "^0.1"),
        ];
        manifest.permissions = vec![Permission::new("cash.read"), Permission::new("cash.write")];
        manifest.nav = vec![NavItem {
            id: "cash".into(),
            label: "Cash Closing".into(),
            href: "/cash".into(),
            permission: Some("cash.read".into()),
            order: 40,
            children: Vec::new(),
        }];
        manifest
    }

    fn migrations(&self) -> &'static [Migration] {
        static MIGRATIONS: &[Migration] = &[Migration::new(
            1,
            "operator_names",
            include_str!("../migrations/001_operator_names.sql"),
        )];
        MIGRATIONS
    }

    fn routes(&self, routes: &mut PluginRoutes) -> Result<(), RegistrationError> {
        routes::mount(routes)
    }

    fn register(&self, registrar: &mut Registrar) {
        registrar.provide(std::sync::Arc::new(CashClosingProvider(
            std::sync::Arc::new(PgCashClosingQuery),
        )));
    }
}

struct PgCashClosingQuery;

#[async_trait]
impl CashClosingQuery for PgCashClosingQuery {
    async fn closing(&self, tx: &mut TenantTx<'_>, id: i32) -> Result<CashClosing, AppError> {
        let shift = desk::load_shift(tx.transaction(), id).await?;
        Ok(CashClosing {
            id: shift.id,
            opened_by: shift.opened_by,
            closed_by: shift.closed_by,
            opened_at: shift.opened_at,
            closed_at: shift.closed_at,
            opening_cash: shift.opening_cash,
            petty_cash: shift.petty_cash,
            sales: shift.sales,
            expenses: shift.expenses,
            cash_taken: shift.cash_taken,
            manual_count: shift.manual_count,
            system_count: shift.system_count,
            difference: shift.difference,
            cash_left: shift.cash_left,
            lines: shift
                .lines
                .into_iter()
                .map(|line| ContractCashExpense {
                    description: line.description,
                    category: line.category,
                    amount: line.amount,
                })
                .collect(),
        })
    }
}
