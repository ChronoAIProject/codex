use super::*;
use crate::history_cell;
use crate::history_cell::HistoryCell;
use pretty_assertions::assert_eq;

#[test]
fn desktop_thread_opened_history_snapshot() {
    let cell = history_cell::new_info_event(
        DESKTOP_THREAD_OPENED_MESSAGE.to_string(),
        /*hint*/ None,
    );

    insta::assert_snapshot!("desktop_thread_opened_history", render_cell(&cell));
}

#[test]
fn desktop_thread_open_error_history_snapshot() {
    let cell = history_cell::new_error_event(desktop_thread_open_error_message("launch failed"));

    insta::assert_snapshot!("desktop_thread_open_error_history", render_cell(&cell));
}

#[test]
fn windows_desktop_app_launch_script_uses_protocol_activation() {
    assert_eq!(
        windows_desktop_app_launch_script("codex://threads/thread-1?label='quoted'"),
        r#"
$ErrorActionPreference = 'Stop'
$url = 'codex://threads/thread-1?label=''quoted'''

$appId = Get-StartApps -Name 'Codex' | Select-Object -First 1 -ExpandProperty AppID
if ([string]::IsNullOrWhiteSpace($appId)) {
    Write-Error 'Codex Desktop package is not installed'
    exit 1
}

Start-Process -FilePath $url
"#
    );
}

fn render_cell(cell: &impl HistoryCell) -> String {
    let lines = cell.display_lines(/*width*/ 80);
    lines
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}
