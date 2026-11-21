/// Named permission string declared by a plugin manifest.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Permission(pub String);

impl Permission {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for Permission {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl std::fmt::Display for Permission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Built-in permissions used by core and the compatibility role map.
pub mod builtins {
    pub const PLATFORM_ADMIN: &str = "platform.admin";
    pub const PLATFORM_PLUGINS: &str = "platform.plugins.manage";
    pub const PLATFORM_TOKENS: &str = "platform.tokens.manage";
    pub const CATALOG_READ: &str = "catalog.read";
    pub const CATALOG_WRITE: &str = "catalog.write";
    pub const INVENTORY_READ: &str = "inventory.read";
    pub const INVENTORY_WRITE: &str = "inventory.write";
    pub const POS_BILL: &str = "pos.bills.post";
    pub const POS_READ: &str = "pos.read";
    pub const CASH_READ: &str = "cash.read";
    pub const CASH_WRITE: &str = "cash.write";
    pub const REPORTS_READ: &str = "reports.read";
    pub const PRINTING_USE: &str = "printing.use";
    pub const SETTINGS_WRITE: &str = "settings.write";
    pub const USERS_WRITE: &str = "users.write";
}
