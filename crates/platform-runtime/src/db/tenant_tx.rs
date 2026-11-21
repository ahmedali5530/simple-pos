use platform_kernel::context::{Actor, TenantId, TenantTx};
use platform_kernel::error::AppError;
use sqlx::PgPool;

/// Extensions for opening tenant transactions from the runtime pool.
pub trait TenantTxExt {
    fn begin_tenant<'a>(
        &'a self,
        tenant_id: TenantId,
        actor: Actor,
    ) -> impl std::future::Future<Output = Result<TenantTx<'a>, AppError>> + Send;

    fn begin_tenant_read<'a>(
        &'a self,
        tenant_id: TenantId,
        actor: Actor,
    ) -> impl std::future::Future<Output = Result<TenantTx<'a>, AppError>> + Send;
}

impl TenantTxExt for PgPool {
    async fn begin_tenant<'a>(
        &'a self,
        tenant_id: TenantId,
        actor: Actor,
    ) -> Result<TenantTx<'a>, AppError> {
        TenantTx::begin_with(self, tenant_id, actor, false).await
    }

    async fn begin_tenant_read<'a>(
        &'a self,
        tenant_id: TenantId,
        actor: Actor,
    ) -> Result<TenantTx<'a>, AppError> {
        TenantTx::begin_with(self, tenant_id, actor, true).await
    }
}
