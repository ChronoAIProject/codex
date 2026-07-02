use std::collections::HashMap;
use std::ffi::OsString;
use std::sync::Arc;
use std::time::Duration;

use codex_rmcp_client::ElicitationAction;
use codex_rmcp_client::ElicitationResponse;
use codex_rmcp_client::LocalStdioServerLauncher;
use codex_rmcp_client::RmcpClient;
use futures::FutureExt as _;
use rmcp::model::ClientCapabilities;
use rmcp::model::Implementation;
use rmcp::model::InitializeRequestParams;
use rmcp::model::ProtocolVersion;
use serde_json::Value;
use serde_json::json;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;

fn init_params() -> InitializeRequestParams {
    InitializeRequestParams::new(
        ClientCapabilities::default(),
        Implementation::new("codex-test", "0.0.0-test").with_title("Codex rmcp stdout noise test"),
    )
    .with_protocol_version(ProtocolVersion::V_2025_06_18)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn stdio_initialize_ignores_startup_stdout_noise() -> anyhow::Result<()> {
    let client = RmcpClient::new_stdio_client(
        std::env::current_exe()?.into(),
        vec![
            OsString::from("stdout_noise_server_child"),
            OsString::from("--exact"),
            OsString::from("--ignored"),
            OsString::from("--nocapture"),
        ],
        Some(HashMap::from([(
            OsString::from("RUST_BACKTRACE"),
            OsString::from("1"),
        )])),
        &[],
        /*cwd*/ None,
        Arc::new(LocalStdioServerLauncher::new(std::env::current_dir()?)),
    )
    .await?;

    client
        .initialize(
            init_params(),
            Some(Duration::from_secs(5)),
            Box::new(|_, _| {
                async {
                    Ok(ElicitationResponse {
                        action: ElicitationAction::Accept,
                        content: Some(json!({})),
                        meta: None,
                    })
                }
                .boxed()
            }),
        )
        .await?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[ignore = "spawned by stdio_initialize_ignores_startup_stdout_noise"]
async fn stdout_noise_server_child() -> anyhow::Result<()> {
    let mut stdin = BufReader::new(tokio::io::stdin()).lines();
    let mut stdout = tokio::io::stdout();
    stdout.write_all(b"server starting\n").await?;
    stdout.flush().await?;

    while let Some(line) = stdin.next_line().await? {
        let message: Value = serde_json::from_str(&line)?;
        if message.get("error").is_some() {
            anyhow::bail!("client sent parse error for startup stdout noise");
        }

        if message.get("method").and_then(Value::as_str) == Some("initialize") {
            let id = message.get("id").cloned().unwrap_or(Value::Null);
            let response = json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "protocolVersion": "2025-06-18",
                    "capabilities": {},
                    "serverInfo": {
                        "name": "stdout-noise-test",
                        "version": "0.0.0-test",
                    },
                },
            });
            stdout.write_all(response.to_string().as_bytes()).await?;
            stdout.write_all(b"\n").await?;
            stdout.flush().await?;
            return Ok(());
        }
    }

    anyhow::bail!("client closed before initialize")
}
