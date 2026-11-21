use async_trait::async_trait;
use platform_kernel::context::TenantTx;
use platform_kernel::error::AppError;
use serde_json::Value;

#[async_trait]
pub trait PrinterDispatch: Send + Sync {
    async fn print_bytes(
        &self,
        tx: &mut TenantTx<'_>,
        bytes: &[u8],
    ) -> Result<bool, AppError>;
}

#[async_trait]
pub trait PrintDocument: Send + Sync {
    fn kind(&self) -> &'static str;
    async fn render(&self, tx: &mut TenantTx<'_>, args: Value) -> Result<Vec<u8>, AppError>;
}
