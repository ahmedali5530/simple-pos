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
    pub const SETTINGS_WRITE: &str = "settings.write";
    pub const USERS_WRITE: &str = "users.write";
}
