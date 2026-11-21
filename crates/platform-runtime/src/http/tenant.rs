use platform_kernel::context::TenantId;

#[derive(Debug, Clone, Copy)]
pub struct RequestTenant(pub TenantId);

impl Default for RequestTenant {
    fn default() -> Self {
        Self(TenantId::DEFAULT)
    }
}
