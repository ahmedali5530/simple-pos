//! Mount the probe plugin, enable it, and hit its route.

use axum::http::StatusCode;
use platform_testkit::{baseline_plugins, boot, probe_ping, provision_and_enable_probe};

#[tokio::test]
async fn probe_route_requires_permission_then_succeeds() {
    let platform = boot(baseline_plugins()).await;
    let tenant_id = provision_and_enable_probe(&platform).await;

    let forbidden = probe_ping(&platform, tenant_id, vec!["core.read".into()]).await;
    assert_eq!(forbidden, StatusCode::FORBIDDEN);

    let ok = probe_ping(&platform, tenant_id, vec!["probe.read".into()]).await;
    assert_eq!(ok, StatusCode::OK);
}
