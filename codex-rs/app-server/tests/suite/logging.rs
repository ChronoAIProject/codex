use anyhow::Context;
use anyhow::Result;
use app_test_support::app_server_json_shutdown_event;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::process::Command;
use std::process::Stdio;
use tempfile::TempDir;

#[test]
fn standalone_app_server_emits_json_info_events() -> Result<()> {
    let codex_home = TempDir::new()?;
    let event = app_server_json_shutdown_event("codex-app-server", &[], codex_home.path())?;

    assert_eq!(
        event,
        json!({
            "level": "INFO",
            "fields": {
                "message": "processor task exited",
                "exit_reason": "last_connection_closed",
                "remaining_connection_count": 0,
                "shutdown_forced": false,
            },
            "target": "codex_app_server",
        })
    );

    Ok(())
}

#[test]
fn project_trust_config_warning_is_not_logged_as_error() -> Result<()> {
    let codex_home = TempDir::new()?;
    let project = TempDir::new()?;
    let dot_codex = project.path().join(".codex");
    std::fs::create_dir(&dot_codex)?;
    std::fs::write(dot_codex.join("config.toml"), r#"foo = "project""#)?;

    std::fs::write(
        codex_home.path().join("config.toml"),
        "[features]\nplugins = false\n",
    )?;
    let output = Command::new(codex_utils_cargo_bin::cargo_bin("codex-app-server")?)
        .stdin(Stdio::null())
        .current_dir(project.path())
        .env("CODEX_HOME", codex_home.path())
        .env(
            "CODEX_APP_SERVER_MANAGED_CONFIG_PATH",
            codex_home.path().join("managed_config.toml"),
        )
        .env("LOG_FORMAT", "json")
        .env("RUST_LOG", "codex_app_server=info")
        .output()?;
    let stderr = String::from_utf8(output.stderr)?;
    anyhow::ensure!(output.status.success(), "app-server failed: {stderr}");
    let events = stderr
        .lines()
        .filter(|line| !line.is_empty())
        .map(serde_json::from_str::<Value>)
        .collect::<serde_json::Result<Vec<_>>>()
        .with_context(|| format!("app-server stderr was not JSONL: {stderr}"))?;
    let event = events
        .iter()
        .find(|event| {
            event["fields"]["message"].as_str().is_some_and(|message| {
                message.contains("Project-local config, hooks, and exec policies are disabled")
            })
        })
        .expect("project trust warning should be logged");

    assert_eq!(event["level"], json!("WARN"));

    Ok(())
}
