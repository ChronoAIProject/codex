use pretty_assertions::assert_eq;
use tracing::Level;
use tracing_subscriber::filter::Targets;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use uuid::Uuid;

use super::*;

#[test]
fn default_filter_caps_payload_heavy_targets_below_trace() {
    let filter = default_filter();

    assert_eq!(
        [
            (
                "codex_client::transport",
                filter.would_enable("codex_client::transport", &Level::TRACE),
            ),
            (
                "codex_api::sse::responses",
                filter.would_enable("codex_api::sse::responses", &Level::TRACE),
            ),
            (
                "codex_state",
                filter.would_enable("codex_state", &Level::TRACE)
            ),
            (
                "codex_client::transport",
                filter.would_enable("codex_client::transport", &Level::DEBUG),
            ),
            (
                "codex_api::sse::responses",
                filter.would_enable("codex_api::sse::responses", &Level::DEBUG),
            ),
        ],
        [
            ("codex_client::transport", false),
            ("codex_api::sse::responses", false),
            ("codex_state", true),
            ("codex_client::transport", true),
            ("codex_api::sse::responses", true),
        ]
    );
}

#[tokio::test]
async fn sqlite_sink_drops_low_level_opentelemetry_sdk_logs() {
    let codex_home =
        std::env::temp_dir().join(format!("codex-state-log-db-filter-{}", Uuid::new_v4()));
    let runtime = StateRuntime::init(codex_home.clone(), "test-provider".to_string())
        .await
        .expect("initialize runtime");
    let layer = start(runtime.clone());

    let guard = tracing_subscriber::registry()
        .with(
            layer
                .clone()
                .with_filter(Targets::new().with_default(tracing::Level::TRACE)),
        )
        .set_default();

    tracing::trace!(target: "opentelemetry_sdk", "dropped-trace");
    tracing::debug!(target: "opentelemetry_sdk", "dropped-debug");
    tracing::info!(target: "opentelemetry_sdk", "retained-info");
    tracing::trace!(target: "codex_state", "retained-trace");

    layer.flush().await;
    drop(guard);

    let logs = runtime
        .query_logs(&crate::LogQuery::default())
        .await
        .expect("query logs after flush");
    assert_eq!(
        logs.iter()
            .map(|row| (
                row.level.as_str(),
                row.target.as_str(),
                row.message.as_deref()
            ))
            .collect::<Vec<_>>(),
        vec![
            ("INFO", "opentelemetry_sdk", Some("retained-info")),
            ("TRACE", "codex_state", Some("retained-trace")),
        ]
    );

    let _ = tokio::fs::remove_dir_all(codex_home).await;
}
