use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::capability::Capability;
use crate::permission::Permission;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavItem {
    pub id: String,
    pub label: String,
    pub href: String,
    pub permission: Option<String>,
    pub order: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsPage {
    pub id: String,
    pub label: String,
    pub href: String,
    pub permission: Option<String>,
    pub order: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportPage {
    pub id: String,
    pub label: String,
    pub href: String,
    pub permission: Option<String>,
    pub order: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    #[serde(default)]
    pub requires: Vec<String>,
    #[serde(default)]
    pub optional: Vec<String>,
    #[serde(default)]
    pub permissions: Vec<Permission>,
    #[serde(default)]
    pub capabilities: Vec<Capability>,
    #[serde(default)]
    pub events_emitted: Vec<String>,
    #[serde(default)]
    pub events_consumed: Vec<String>,
    #[serde(default)]
    pub settings_schema: Option<Value>,
    #[serde(default)]
    pub nav: Vec<NavItem>,
    #[serde(default)]
    pub settings_pages: Vec<SettingsPage>,
    #[serde(default)]
    pub report_pages: Vec<ReportPage>,
}
