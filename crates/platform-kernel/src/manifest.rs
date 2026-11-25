use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::capability::Capability;
use crate::permission::Permission;

/// Schema version of [`PluginManifest`]. Version 1 manifests stay readable.
pub const MANIFEST_VERSION: u32 = 2;

/// Kernel API this binary implements. Plugins declare a compatible range.
pub fn kernel_api_version() -> Version {
    Version::new(1, 0, 0)
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum ManifestError {
    #[error(
        "plugin id `{0}` is invalid; use 2–32 characters of [a-z0-9_], starting with a letter"
    )]
    InvalidId(String),
    #[error("unsupported manifest version {0}")]
    UnsupportedVersion(u32),
    #[error("plugin `{plugin}` requires kernel API `{required}`, but this kernel is {actual}")]
    IncompatibleKernel {
        plugin: String,
        required: String,
        actual: String,
    },
    #[error("plugin `{plugin}` cannot declare permission `{permission}`")]
    ForeignPermission { plugin: String, permission: String },
    #[error("plugin `{plugin}` capability `{capability}` has an invalid id or undeclared permission `{permission}`")]
    InvalidCapability {
        plugin: String,
        capability: String,
        permission: String,
    },
    #[error("plugin `{plugin}` {direction} consent needs a title and a body")]
    EmptyConsent {
        plugin: String,
        direction: &'static str,
    },
}

/// Copy the operator must accept before a plugin is switched.
/// A plugin omits this when that direction needs no extra confirmation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SwitchConsent {
    pub title: String,
    pub body: String,
    /// The operator must tick an acknowledgement. Used when the plugin shares data outside the shop.
    #[serde(default)]
    pub acknowledge: bool,
}

/// Optional confirmations for turning a plugin on and off. Either side can be absent.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginConsents {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enable: Option<SwitchConsent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disable: Option<SwitchConsent>,
}

fn consents_are_empty(consents: &PluginConsents) -> bool {
    consents.enable.is_none() && consents.disable.is_none()
}

/// Plugin identifier. Serialized as a plain string.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PluginId(String);

impl PluginId {
    pub fn parse(value: &str) -> Result<Self, ManifestError> {
        let ok = (2..=32).contains(&value.len())
            && value.starts_with(|c: char| c.is_ascii_lowercase())
            && value
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
        if ok {
            Ok(Self(value.to_string()))
        } else {
            Err(ManifestError::InvalidId(value.to_string()))
        }
    }

    /// For compiled-in manifests whose ids are known to be valid.
    pub fn must(value: &str) -> Self {
        Self::parse(value).unwrap_or_else(|err| panic!("{err}"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for PluginId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A required or optional dependency with a semver range.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dependency {
    pub id: PluginId,
    pub version: VersionReq,
}

impl Dependency {
    pub fn new(id: &str, version: &str) -> Self {
        Self {
            id: PluginId::must(id),
            version: VersionReq::parse(version)
                .unwrap_or_else(|err| panic!("version requirement `{version}`: {err}")),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NavItem {
    pub id: String,
    pub label: String,
    /// Empty when this entry is only a group that opens its `children`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub href: String,
    #[serde(default)]
    pub permission: Option<String>,
    #[serde(default)]
    pub order: i32,
    /// Sub-links rendered as a dropdown under this entry.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<NavItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsPage {
    pub id: String,
    pub label: String,
    /// Empty when this entry is only a group that opens its `children`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub href: String,
    pub permission: Option<String>,
    pub order: i32,
    /// Nested settings links under this group (e.g. Catalog → Items).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<SettingsPage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportPage {
    pub id: String,
    pub label: String,
    pub href: String,
    pub permission: Option<String>,
    pub order: i32,
}

/// A screen the plugin contributes. The shell renders it; the kernel does not.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageDecl {
    pub path: String,
    pub title: String,
    pub permission: Option<String>,
    /// `compact` or `touch`. Empty means the shell default.
    #[serde(default)]
    pub density: String,
}

/// A UI fragment rendered inside another plugin's screen.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlotDecl {
    pub name: String,
    pub contribution: String,
    #[serde(default)]
    pub order: i32,
}

/// Permissions a plugin may declare that are not under its own id prefix.
const SHARED_PERMISSIONS: &[&str] = &["users.write", "settings.write"];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PluginManifest {
    pub manifest_version: u32,
    pub id: PluginId,
    pub name: String,
    pub version: Version,
    /// Semver range of kernel API versions this plugin supports.
    pub kernel_api: VersionReq,
    pub description: String,
    #[serde(default)]
    pub requires: Vec<Dependency>,
    #[serde(default)]
    pub optional: Vec<Dependency>,
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
    /// Whether a brand-new tenant should start with this plugin enabled.
    #[serde(default)]
    pub default_enabled: bool,
    #[serde(default)]
    pub nav: Vec<NavItem>,
    #[serde(default)]
    pub settings_pages: Vec<SettingsPage>,
    #[serde(default)]
    pub report_pages: Vec<ReportPage>,
    #[serde(default)]
    pub pages: Vec<PageDecl>,
    #[serde(default)]
    pub slots_provided: Vec<String>,
    #[serde(default)]
    pub slot_contributions: Vec<SlotDecl>,
    /// Confirmations shown before the plugin is turned on or off.
    #[serde(default, skip_serializing_if = "consents_are_empty")]
    pub consents: PluginConsents,
}

impl PluginManifest {
    /// A valid v1 manifest. Panics if `id` or `version` is not valid — compiled-in data only.
    pub fn v1(id: &str, name: impl Into<String>, version: &str) -> Self {
        Self {
            manifest_version: 1,
            id: PluginId::must(id),
            name: name.into(),
            version: Version::parse(version)
                .unwrap_or_else(|err| panic!("plugin version `{version}`: {err}")),
            kernel_api: VersionReq::parse("^1.0").expect("kernel api range"),
            description: String::new(),
            requires: Vec::new(),
            optional: Vec::new(),
            permissions: Vec::new(),
            capabilities: Vec::new(),
            events_emitted: Vec::new(),
            events_consumed: Vec::new(),
            settings_schema: None,
            default_enabled: false,
            nav: Vec::new(),
            settings_pages: Vec::new(),
            report_pages: Vec::new(),
            pages: Vec::new(),
            slots_provided: Vec::new(),
            slot_contributions: Vec::new(),
            consents: PluginConsents::default(),
        }
    }

    pub fn validate(&self) -> Result<(), ManifestError> {
        PluginId::parse(self.id.as_str())?;
        if self.manifest_version != 1 && self.manifest_version != MANIFEST_VERSION {
            return Err(ManifestError::UnsupportedVersion(self.manifest_version));
        }
        if !self.kernel_api.matches(&kernel_api_version()) {
            return Err(ManifestError::IncompatibleKernel {
                plugin: self.id.to_string(),
                required: self.kernel_api.to_string(),
                actual: kernel_api_version().to_string(),
            });
        }
        let prefix = format!("{}.", self.id.as_str());
        for permission in &self.permissions {
            let name = permission.as_str();
            if name.starts_with(&prefix) || SHARED_PERMISSIONS.contains(&name) {
                continue;
            }
            return Err(ManifestError::ForeignPermission {
                plugin: self.id.to_string(),
                permission: name.to_string(),
            });
        }
        let declared: std::collections::HashSet<_> =
            self.permissions.iter().map(|p| p.as_str()).collect();
        let mut capability_ids = std::collections::HashSet::new();
        let prefix = format!("{}.", self.id.as_str());
        for capability in &self.capabilities {
            if !capability.id.starts_with(&prefix)
                || !capability_ids.insert(capability.id.as_str())
                || !declared.contains(capability.permission.as_str())
            {
                return Err(ManifestError::InvalidCapability {
                    plugin: self.id.to_string(),
                    capability: capability.id.clone(),
                    permission: capability.permission.clone(),
                });
            }
        }
        for (direction, consent) in [
            ("enable", &self.consents.enable),
            ("disable", &self.consents.disable),
        ] {
            if let Some(consent) = consent {
                if consent.title.trim().is_empty() || consent.body.trim().is_empty() {
                    return Err(ManifestError::EmptyConsent {
                        plugin: self.id.to_string(),
                        direction,
                    });
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::permission::Permission;

    #[test]
    fn rejects_bad_plugin_id() {
        assert!(PluginId::parse("Catalog").is_err());
        assert!(PluginId::parse("a").is_err());
        assert!(PluginId::parse("1catalog").is_err());
        assert!(PluginId::parse("catalog").is_ok());
    }

    #[test]
    fn rejects_unsupported_manifest_version() {
        let mut manifest = PluginManifest::v1("catalog", "Catalog", "0.1.0");
        manifest.manifest_version = 99;
        assert!(matches!(
            manifest.validate(),
            Err(ManifestError::UnsupportedVersion(99))
        ));
    }

    #[test]
    fn rejects_incompatible_kernel_api() {
        let mut manifest = PluginManifest::v1("catalog", "Catalog", "0.1.0");
        manifest.kernel_api = VersionReq::parse("^2.0").unwrap();
        assert!(matches!(
            manifest.validate(),
            Err(ManifestError::IncompatibleKernel { .. })
        ));
    }

    #[test]
    fn rejects_foreign_permission_namespace() {
        let mut manifest = PluginManifest::v1("catalog", "Catalog", "0.1.0");
        manifest.permissions = vec![Permission::new("inventory.write")];
        assert!(matches!(
            manifest.validate(),
            Err(ManifestError::ForeignPermission { .. })
        ));
    }

    #[test]
    fn allows_own_namespace_and_shared_permissions() {
        let mut manifest = PluginManifest::v1("catalog", "Catalog", "0.1.0");
        manifest.permissions = vec![
            Permission::new("catalog.read"),
            Permission::new("users.write"),
            Permission::new("settings.write"),
        ];
        assert!(manifest.validate().is_ok());
    }

    #[test]
    fn nav_items_carry_children_and_groups_need_no_href() {
        let json = serde_json::json!({
            "id": "mobile",
            "label": "Mobile",
            "permission": "mobile.read",
            "order": 48,
            "children": [
                { "id": "mobile_devices", "label": "Devices", "href": "/mobile/devices" },
                { "id": "mobile_warranty", "label": "Warranty", "href": "/mobile/warranty" }
            ]
        });
        let entry: NavItem = serde_json::from_value(json).unwrap();
        assert!(entry.href.is_empty());
        assert_eq!(entry.children.len(), 2);
        assert_eq!(entry.children[1].href, "/mobile/warranty");

        let round_trip = serde_json::to_value(&entry).unwrap();
        assert_eq!(round_trip["children"].as_array().unwrap().len(), 2);
        assert!(
            round_trip.get("href").is_none(),
            "an empty group href is omitted"
        );
    }

    #[test]
    fn nav_items_without_children_omit_the_key() {
        let entry = NavItem {
            id: "billing".into(),
            label: "Billing".into(),
            href: "/billing".into(),
            permission: None,
            order: 10,
            children: Vec::new(),
        };
        let value = serde_json::to_value(&entry).unwrap();
        assert!(value.get("children").is_none());
    }

    #[test]
    fn consents_are_optional_and_each_direction_stands_alone() {
        let mut manifest = PluginManifest::v1("filings", "Filings", "0.1.0");
        assert!(manifest.validate().is_ok());
        manifest.consents.enable = Some(SwitchConsent {
            title: "Connect this shop".into(),
            body: "Turning this on shares filings with the authority named by the plugin.".into(),
            acknowledge: true,
        });
        assert!(manifest.validate().is_ok());
        manifest.consents.enable = Some(SwitchConsent {
            title: " ".into(),
            body: "Body".into(),
            acknowledge: false,
        });
        assert!(matches!(
            manifest.validate(),
            Err(ManifestError::EmptyConsent {
                direction: "enable",
                ..
            })
        ));
    }
}
