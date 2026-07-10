use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use codex_code_mode_protocol::CodeModeSessionProvider;
#[cfg(unix)]
use std::os::unix::fs::symlink;

use super::ProcessOwnedCodeModeSession;
use super::ProcessOwnedCodeModeSessionProvider;
use super::resolve_host_program;
use crate::NoopCodeModeSessionDelegate;

#[test]
fn provider_reuses_its_live_process_host() {
    let provider = ProcessOwnedCodeModeSessionProvider::default();

    let first = provider.process_host();
    let second = provider.process_host();

    assert!(Arc::ptr_eq(&first, &second));
}

#[test]
fn host_program_override_takes_precedence() {
    assert_eq!(
        resolve_host_program(
            Some("custom-code-mode-host".into()),
            Ok(PathBuf::from("/opt/codex/bin/codex")),
        ),
        PathBuf::from("custom-code-mode-host")
    );
}

#[test]
fn host_program_is_next_to_the_main_executable_even_when_missing() {
    let executable_name = if cfg!(windows) {
        "codex-code-mode-host.exe"
    } else {
        "codex-code-mode-host"
    };

    assert_eq!(
        resolve_host_program(
            /*override_path*/ None,
            Ok(PathBuf::from("/opt/codex/bin/codex")),
        ),
        PathBuf::from("/opt/codex/bin").join(executable_name)
    );
}

#[cfg(unix)]
#[test]
fn host_program_follows_main_executable_symlink() -> io::Result<()> {
    let executable_name = "codex-code-mode-host";
    let unique_suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "codex-code-mode-host-symlink-{}-{unique_suffix}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&temp_dir);
    let release_bin_dir = temp_dir.join("release/bin");
    let visible_bin_dir = temp_dir.join("visible/bin");
    std::fs::create_dir_all(&release_bin_dir)?;
    std::fs::create_dir_all(&visible_bin_dir)?;
    let release_codex = release_bin_dir.join("codex");
    std::fs::write(&release_codex, "")?;
    let visible_codex = visible_bin_dir.join("codex");
    symlink(&release_codex, &visible_codex)?;
    let expected_host = release_bin_dir.canonicalize()?.join(executable_name);

    assert_eq!(
        resolve_host_program(/*override_path*/ None, Ok(visible_codex)),
        expected_host
    );

    std::fs::remove_dir_all(&temp_dir)?;

    Ok(())
}

#[test]
fn host_program_falls_back_to_its_name_when_main_executable_is_unknown() {
    let executable_name = if cfg!(windows) {
        "codex-code-mode-host.exe"
    } else {
        "codex-code-mode-host"
    };

    assert_eq!(
        resolve_host_program(
            /*override_path*/ None,
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                "missing executable"
            )),
        ),
        PathBuf::from(executable_name)
    );
}

#[tokio::test]
async fn provider_reports_host_spawn_failure() {
    let provider = ProcessOwnedCodeModeSessionProvider::with_host_program(
        "codex-code-mode-host-does-not-exist".into(),
    );

    let error = provider
        .create_session(Arc::new(NoopCodeModeSessionDelegate))
        .await
        .err()
        .expect("session creation should fail");

    assert!(error.contains("failed to spawn code-mode host"));
}

#[tokio::test]
async fn shutdown_before_open_does_not_spawn_the_host() {
    let session = ProcessOwnedCodeModeSession::new();

    session.shutdown().await.expect("shutdown session");
    let error = session
        .execute(codex_code_mode_protocol::ExecuteRequest {
            tool_call_id: "call-1".to_string(),
            enabled_tools: Vec::new(),
            source: "text('unreachable')".to_string(),
            yield_time_ms: None,
            max_output_tokens: None,
        })
        .await
        .err()
        .expect("shutdown session should reject execution");

    assert_eq!(error, "code mode session is shutting down");
}
