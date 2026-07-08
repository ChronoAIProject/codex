use std::process::Stdio;
use std::sync::Arc;

use serde::Serialize;

use crate::Hook;
use crate::HookEvent;
use crate::HookPayload;
use crate::HookResult;
use crate::command_from_argv;

const LEGACY_NOTIFY_ARG_MAX_BYTES: usize = 8 * 1024;
const LEGACY_NOTIFY_MESSAGE_MAX_BYTES: usize = 2 * 1024;
const TRUNCATED_MARKER: &str = "...";

/// Legacy notify payload appended as the final argv argument for backward compatibility.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
enum UserNotification {
    #[serde(rename_all = "kebab-case")]
    AgentTurnComplete {
        thread_id: String,
        turn_id: String,
        cwd: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        client: Option<String>,
        input_messages: Vec<String>,
        last_assistant_message: Option<String>,
    },
}

pub fn legacy_notify_json(payload: &HookPayload) -> Result<String, serde_json::Error> {
    match &payload.hook_event {
        HookEvent::AfterAgent { event } => {
            serde_json::to_string(&UserNotification::AgentTurnComplete {
                thread_id: event.thread_id.to_string(),
                turn_id: event.turn_id.clone(),
                cwd: payload.cwd.display().to_string(),
                client: payload.client.clone(),
                input_messages: event.input_messages.clone(),
                last_assistant_message: event.last_assistant_message.clone(),
            })
        }
    }
}

fn truncate_for_argv(value: &str) -> String {
    if value.len() <= LEGACY_NOTIFY_MESSAGE_MAX_BYTES {
        return value.to_string();
    }

    let max_prefix_bytes = LEGACY_NOTIFY_MESSAGE_MAX_BYTES.saturating_sub(TRUNCATED_MARKER.len());
    let mut prefix_len = 0;
    for (idx, ch) in value.char_indices() {
        let char_end = idx + ch.len_utf8();
        if char_end > max_prefix_bytes {
            break;
        }
        prefix_len = char_end;
    }

    format!("{}{}", &value[..prefix_len], TRUNCATED_MARKER)
}

fn bounded_legacy_notify_json(payload: &HookPayload) -> Result<String, serde_json::Error> {
    match &payload.hook_event {
        HookEvent::AfterAgent { event } => {
            let thread_id = event.thread_id.to_string();
            let turn_id = event.turn_id.clone();
            let cwd = payload.cwd.display().to_string();
            let client = payload.client.clone();
            let mut input_messages = event
                .input_messages
                .iter()
                .map(|message| truncate_for_argv(message))
                .collect::<Vec<_>>();
            let mut last_assistant_message = event
                .last_assistant_message
                .as_ref()
                .map(|message| truncate_for_argv(message));

            let serialize = |input_messages: Vec<String>,
                             last_assistant_message: Option<String>|
             -> Result<String, serde_json::Error> {
                serde_json::to_string(&UserNotification::AgentTurnComplete {
                    thread_id: thread_id.clone(),
                    turn_id: turn_id.clone(),
                    cwd: cwd.clone(),
                    client: client.clone(),
                    input_messages,
                    last_assistant_message,
                })
            };

            let mut serialized = serialize(input_messages.clone(), last_assistant_message.clone())?;
            if serialized.len() <= LEGACY_NOTIFY_ARG_MAX_BYTES {
                return Ok(serialized);
            }

            input_messages.truncate(1);
            serialized = serialize(input_messages.clone(), last_assistant_message.clone())?;
            if serialized.len() <= LEGACY_NOTIFY_ARG_MAX_BYTES {
                return Ok(serialized);
            }

            input_messages.clear();
            serialized = serialize(input_messages.clone(), last_assistant_message.clone())?;
            if serialized.len() <= LEGACY_NOTIFY_ARG_MAX_BYTES {
                return Ok(serialized);
            }

            last_assistant_message = None;
            serialize(input_messages, last_assistant_message)
        }
    }
}

pub fn notify_hook(argv: Vec<String>) -> Hook {
    let argv = Arc::new(argv);
    Hook {
        name: "legacy_notify".to_string(),
        func: Arc::new(move |payload: &HookPayload| {
            let argv = Arc::clone(&argv);
            Box::pin(async move {
                let mut command = match command_from_argv(&argv) {
                    Some(command) => command,
                    None => return HookResult::Success,
                };
                if let Ok(notify_payload) = bounded_legacy_notify_json(payload) {
                    command.arg(notify_payload);
                }

                command
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null());

                match command.spawn() {
                    Ok(_) => HookResult::Success,
                    Err(err) => HookResult::FailedContinue(err.into()),
                }
            })
        }),
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use codex_protocol::ThreadId;
    use codex_utils_absolute_path::test_support::PathBufExt;
    use codex_utils_absolute_path::test_support::test_path_buf;
    use pretty_assertions::assert_eq;
    use serde_json::Value;
    use serde_json::json;

    use super::*;
    use crate::HookEventAfterAgent;

    fn expected_notification_json() -> Value {
        let cwd = test_path_buf("/Users/example/project");
        json!({
            "type": "agent-turn-complete",
            "thread-id": "b5f6c1c2-1111-2222-3333-444455556666",
            "turn-id": "12345",
            "cwd": cwd.display().to_string(),
            "client": "codex-tui",
            "input-messages": ["Rename `foo` to `bar` and update the callsites."],
            "last-assistant-message": "Rename complete and verified `cargo build` succeeds.",
        })
    }

    #[test]
    fn test_user_notification() -> Result<()> {
        let notification = UserNotification::AgentTurnComplete {
            thread_id: "b5f6c1c2-1111-2222-3333-444455556666".to_string(),
            turn_id: "12345".to_string(),
            cwd: test_path_buf("/Users/example/project")
                .display()
                .to_string(),
            client: Some("codex-tui".to_string()),
            input_messages: vec!["Rename `foo` to `bar` and update the callsites.".to_string()],
            last_assistant_message: Some(
                "Rename complete and verified `cargo build` succeeds.".to_string(),
            ),
        };
        let serialized = serde_json::to_string(&notification)?;
        let actual: Value = serde_json::from_str(&serialized)?;
        assert_eq!(actual, expected_notification_json());
        Ok(())
    }

    #[test]
    fn legacy_notify_json_matches_historical_wire_shape() -> Result<()> {
        let payload = HookPayload {
            session_id: ThreadId::new(),
            cwd: test_path_buf("/Users/example/project").abs(),
            client: Some("codex-tui".to_string()),
            triggered_at: chrono::Utc::now(),
            hook_event: HookEvent::AfterAgent {
                event: HookEventAfterAgent {
                    thread_id: ThreadId::from_string("b5f6c1c2-1111-2222-3333-444455556666")
                        .expect("valid thread id"),
                    turn_id: "12345".to_string(),
                    input_messages: vec![
                        "Rename `foo` to `bar` and update the callsites.".to_string(),
                    ],
                    last_assistant_message: Some(
                        "Rename complete and verified `cargo build` succeeds.".to_string(),
                    ),
                },
            },
        };

        let serialized = legacy_notify_json(&payload)?;
        let actual: Value = serde_json::from_str(&serialized)?;
        assert_eq!(actual, expected_notification_json());

        Ok(())
    }

    #[test]
    fn bounded_legacy_notify_json_caps_large_argv_payload() -> Result<()> {
        let payload = HookPayload {
            session_id: ThreadId::new(),
            cwd: test_path_buf("/Users/example/project").abs(),
            client: Some("codex-app".to_string()),
            triggered_at: chrono::Utc::now(),
            hook_event: HookEvent::AfterAgent {
                event: HookEventAfterAgent {
                    thread_id: ThreadId::from_string("b5f6c1c2-1111-2222-3333-444455556666")
                        .expect("valid thread id"),
                    turn_id: "12345".to_string(),
                    input_messages: vec!["user input ".repeat(16 * 1024); 4],
                    last_assistant_message: Some("assistant output ".repeat(16 * 1024)),
                },
            },
        };

        let serialized = bounded_legacy_notify_json(&payload)?;
        let actual: Value = serde_json::from_str(&serialized)?;

        assert!(serialized.len() <= LEGACY_NOTIFY_ARG_MAX_BYTES);
        assert_eq!(actual["type"], json!("agent-turn-complete"));
        assert_eq!(
            actual["thread-id"],
            json!("b5f6c1c2-1111-2222-3333-444455556666")
        );
        assert_eq!(actual["turn-id"], json!("12345"));
        assert_eq!(
            actual["cwd"],
            json!(
                test_path_buf("/Users/example/project")
                    .display()
                    .to_string()
            )
        );
        assert_eq!(actual["client"], json!("codex-app"));
        let input_messages = actual["input-messages"]
            .as_array()
            .expect("input messages is an array");
        assert!(input_messages.len() <= 1);
        assert!(input_messages.iter().all(|message| {
            message
                .as_str()
                .is_some_and(|message| message.len() <= LEGACY_NOTIFY_MESSAGE_MAX_BYTES)
        }));
        assert!(
            actual["last-assistant-message"]
                .as_str()
                .is_none_or(|message| message.len() <= LEGACY_NOTIFY_MESSAGE_MAX_BYTES)
        );
        assert!(
            legacy_notify_json(&payload)?.len() > LEGACY_NOTIFY_ARG_MAX_BYTES,
            "test payload should reproduce an overlarge legacy argv argument"
        );
        assert_ne!(
            actual,
            json!({
                "type": "agent-turn-complete",
                "thread-id": "b5f6c1c2-1111-2222-3333-444455556666",
                "turn-id": "12345",
                "cwd": test_path_buf("/Users/example/project").display().to_string(),
                "client": "codex-app",
                "input-messages": vec!["user input ".repeat(16 * 1024); 4],
                "last-assistant-message": "assistant output ".repeat(16 * 1024),
            })
        );

        Ok(())
    }
}
