/// A single platform or plugin migration.
#[derive(Debug, Clone)]
pub struct Migration {
    pub version: i64,
    pub name: &'static str,
    pub sql: &'static str,
}

impl Migration {
    pub const fn new(version: i64, name: &'static str, sql: &'static str) -> Self {
        Self {
            version,
            name,
            sql,
        }
    }

    pub fn checksum(&self) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        self.version.hash(&mut hasher);
        self.name.hash(&mut hasher);
        self.sql.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }
}
