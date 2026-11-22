use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;

use crate::context::TenantTx;
use crate::error::AppError;
use crate::event::{EventEnvelope, EventHandler};
use crate::manifest::PluginId;
use crate::routes::RegistrationError;

/// The only way a plugin may subscribe to domain events.
///
/// A handler is accepted only when its event type is listed in `events_consumed`.
/// The subscriber id is forced to `{plugin_id}:{name}` so delivery can be paused
/// while the plugin is disabled.
pub struct PluginEvents {
    plugin_id: PluginId,
    consumed: HashSet<String>,
    handlers: Vec<Arc<dyn EventHandler>>,
}

impl PluginEvents {
    pub fn new(plugin_id: PluginId, consumed: HashSet<String>) -> Self {
        Self {
            plugin_id,
            consumed,
            handlers: Vec::new(),
        }
    }

    pub fn on(&mut self, handler: Arc<dyn EventHandler>) -> Result<(), RegistrationError> {
        if !self.consumed.contains(handler.event_type()) {
            return Err(RegistrationError::UndeclaredEvent {
                plugin: self.plugin_id.to_string(),
                event: handler.event_type().to_string(),
            });
        }
        let subscriber_id = format!("{}:{}", self.plugin_id, handler.subscriber_id());
        self.handlers.push(Arc::new(BoundHandler {
            owner: self.plugin_id.to_string(),
            subscriber_id,
            event_type: handler.event_type().to_string(),
            inner: handler,
        }));
        Ok(())
    }

    pub fn into_handlers(self) -> Vec<Arc<dyn EventHandler>> {
        self.handlers
    }
}

struct BoundHandler {
    owner: String,
    subscriber_id: String,
    event_type: String,
    inner: Arc<dyn EventHandler>,
}

#[async_trait]
impl EventHandler for BoundHandler {
    fn event_type(&self) -> &str {
        &self.event_type
    }

    fn subscriber_id(&self) -> &str {
        &self.subscriber_id
    }

    fn owner_plugin(&self) -> Option<&str> {
        Some(&self.owner)
    }

    async fn handle(&self, tx: &mut TenantTx<'_>, envelope: &EventEnvelope) -> Result<(), AppError> {
        self.inner.handle(tx, envelope).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Probe;

    #[async_trait]
    impl EventHandler for Probe {
        fn event_type(&self) -> &str {
            "sales.posted"
        }
        fn subscriber_id(&self) -> &str {
            "probe"
        }
        async fn handle(&self, _: &mut TenantTx<'_>, _: &EventEnvelope) -> Result<(), AppError> {
            Ok(())
        }
    }

    #[test]
    fn rejects_events_the_manifest_does_not_consume() {
        let mut events = PluginEvents::new(PluginId::must("inventory"), HashSet::new());
        let err = events.on(Arc::new(Probe)).unwrap_err();
        assert!(matches!(err, RegistrationError::UndeclaredEvent { .. }));
    }

    #[test]
    fn forces_subscriber_id_under_the_plugin() {
        let mut consumed = HashSet::new();
        consumed.insert("sales.posted".into());
        let mut events = PluginEvents::new(PluginId::must("inventory"), consumed);
        events.on(Arc::new(Probe)).unwrap();
        let handlers = events.into_handlers();
        assert_eq!(handlers[0].subscriber_id(), "inventory:probe");
        assert_eq!(handlers[0].owner_plugin(), Some("inventory"));
    }
}
