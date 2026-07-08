use std::process::Command;

use anyhow::Result;
use app_test_support::TestAppServer;
use tempfile::TempDir;
use tokio::time::Duration;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(10);

#[test]
fn strict_config_rejects_unknown_config_fields_for_standalone_app_server() -> Result<()> {
    let codex_home = TempDir::new()?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        r#"
foo = "bar"
"#,
    )?;

    let output = Command::new(codex_utils_cargo_bin::cargo_bin("codex-app-server")?)
        .env("CODEX_HOME", codex_home.path())
        .env(
            "CODEX_APP_SERVER_MANAGED_CONFIG_PATH",
            codex_home.path().join("managed_config.toml"),
        )
        .args(["--strict-config", "--listen", "off"])
        .output()?;

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr)?;
    assert!(
        stderr.contains("unknown configuration field `foo`"),
        "expected strict config error in stderr, got: {stderr}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn invalid_otel_metrics_exporter_does_not_block_standalone_app_server_startup() -> Result<()>
{
    let codex_home = TempDir::new()?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        r#"
[analytics]
enabled = true
"#,
    )?;

    let mut app_server = TestAppServer::new_with_env(
        codex_home.path(),
        &[
            ("OTEL_METRICS_EXPORTER", Some("otlp")),
            ("OTEL_EXPORTER_OTLP_ENDPOINT", Some("http://127.0.0.1:4317")),
            ("OTEL_EXPORTER_OTLP_PROTOCOL", Some("grpc")),
            ("OTEL_EXPORTER_OTLP_COMPRESSION", Some("snappy")),
        ],
    )
    .await?;
    timeout(DEFAULT_READ_TIMEOUT, app_server.initialize()).await??;

    Ok(())
}
