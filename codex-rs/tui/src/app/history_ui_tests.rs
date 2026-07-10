use super::*;
use crate::history_cell;
use crate::history_cell::HistoryCell;

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
fn windows_desktop_app_launch_uses_protocol_handler() {
    let script = windows_desktop_app_launch_script("codex://threads/thread'with-quote");

    assert!(script.contains("Get-StartApps -Name 'Codex'"));
    assert!(script.contains("Start-Process -FilePath $url"));
    assert!(script.contains("$url = 'codex://threads/thread''with-quote'"));
    assert!(!script.contains("Get-AppxPackage"));
    assert!(!script.contains("Codex.exe"));
    assert!(!script.contains("resources\\app.asar"));
}

fn render_cell(cell: &impl HistoryCell) -> String {
    let lines = cell.display_lines(/*width*/ 80);
    lines
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}
