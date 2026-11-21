use async_trait::async_trait;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::context::{TenantId, TenantTx};
use crate::error::AppError;

pub trait DomainEvent: Serialize + DeserializeOwned + Send + Sync {
    const TYPE: &'static str;
    const VERSION: u16;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope {
    pub id: Uuid,
    pub tenant_id: TenantId,
    pub event_type: String,
    pub version: u16,
    pub payload: Value,
    pub created_at: chrono_lite::DateTime,
}

/// Minimal chrono-free timestamp wrapper so kernel stays light.
/// Runtime fills this from the database.
pub mod chrono_lite {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct DateTime(pub String);

    impl DateTime {
        pub fn now() -> Self {
            Self(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs().to_string())
                    .unwrap_or_default(),
            )
        }
    }
}

#[async_trait]
pub trait EventHandler: Send + Sync {
    fn event_type(&self) -> &str;
    fn subscriber_id(&self) -> &str;
    async fn handle(&self, tx: &mut TenantTx<'_>, envelope: &EventEnvelope) -> Result<(), AppError>;
}
