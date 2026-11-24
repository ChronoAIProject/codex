//! Cancellation prevents new speculation without cancelling session-owned work.

use super::*;
use codex_analytics::CompactionReason;
use codex_analytics::CompactionTrigger;
use codex_login::CodexAuth;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::TokenUsage;
use core_test_support::responses;
use core_test_support::streaming_sse::StreamingSseChunk;
use core_test_support::streaming_sse::start_streaming_sse_server;
use pretty_assertions::assert_eq;
use tokio::time::Duration;
use tokio::time::timeout;

use crate::session::tests::HeldStepTask;
use crate::state::TaskKind;

#[test_case::test_case(CompactionTrigger::Auto; "optional ready adoption")]
#[test_case::test_case(CompactionTrigger::Manual; "manual ready reuse")]
#[tokio::test]
async fn cancelled_turn_skips_launch_but_preserves_existing_work(trigger: CompactionTrigger) {
    let phase = CompactionPhase::MidTurn;
    let server = responses::start_mock_server().await;
    let (sess, turn, _events) = compaction_session(server.uri()).await;
    let request = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            serde_json::json!({"type":"response.output_item.done", "item": {
                "type":"compaction", "encrypted_content":"prepared-summary"
            }}),
            responses::ev_completed("prepared"),
        ]),
    )
    .await;
    let turn_cancellation = CancellationToken::new();
    let step = sess
        .capture_step_context(Arc::clone(&turn), &turn_cancellation)
        .await
        .expect("capture step");
    let original: ResponseItem = serde_json::from_value(serde_json::json!({
        "type": "message", "role": "user",
        "content": [{"type": "input_text", "text": "original turn"}]
    }))
    .expect("user message");
    sess.replace_history(vec![original], /*reference_context_item*/ None)
        .await;
    sess.update_token_usage_info(
        &turn,
        Some(&TokenUsage {
            input_tokens: 70_000,
            total_tokens: 70_000,
            ..Default::default()
        }),
    )
    .await
    .expect("record usage");
    let status = super::super::context_window::context_window_token_status(&sess, &turn).await;
    let world_state = Arc::new(WorldState::default());
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    let auth = sess.services.model_client.auth_owner_generation();
    maybe_start(
        &sess,
        &step,
        &world_state,
        phase,
        &cancelled,
        &status,
        (Some("routing".to_string()), auth),
    )
    .await;
    assert!(
        sess.state
            .lock()
            .await
            .async_compaction
            .last_attempt
            .is_none()
    );
    assert_eq!(request.requests().len(), 0);

    // The same eligible work can launch on a live turn, then outlive its cancellation.
    maybe_start(
        &sess,
        &step,
        &world_state,
        phase,
        &turn_cancellation,
        &status,
        (Some("routing".to_string()), auth),
    )
    .await;
    turn_cancellation.cancel();
    maybe_start(
        &sess,
        &step,
        &world_state,
        phase,
        &turn_cancellation,
        &status,
        (Some("routing".to_string()), auth),
    )
    .await;
    // Wait for client publication before exercising either ready-result consumer.
    wait_for_background_compaction_for_test(&sess).await;
    let next_step = sess
        .capture_step_context(Arc::clone(&turn), &CancellationToken::new())
        .await
        .expect("capture next step");
    match trigger {
        CompactionTrigger::Auto => {
            let candidate = take_ready(&sess, &turn).await.expect("published candidate");
            let mut client_session = sess.services.model_client.new_session();
            assert!(
                crate::compact_remote_v2::run_inline_remote_auto_compact_task(
                    Arc::clone(&sess),
                    Arc::clone(&next_step),
                    next_step,
                    &mut client_session,
                    world_state,
                    CompactionReason::ContextLimit,
                    phase,
                    Some(candidate),
                    crate::compact_remote_v2::CompactionFallback::Skip,
                )
                .await
                .expect("optional adoption")
            );
        }
        CompactionTrigger::Manual => {
            crate::compact_remote_v2::run_remote_compact_task(
                Arc::clone(&sess),
                next_step,
                world_state,
            )
            .await
            .expect("manual compaction reuses the completed request");
        }
    }
    let history = sess.clone_history().await;
    let summaries = history
        .raw_items()
        .filter_map(|item| match item {
            ResponseItem::Compaction {
                encrypted_content, ..
            } => Some(encrypted_content.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(summaries, ["prepared-summary"]);
    assert_eq!(request.requests().len(), 1);
    shutdown(&sess).await;
}

async fn compaction_session(
    server_uri: String,
) -> (
    Arc<Session>,
    Arc<TurnContext>,
    async_channel::Receiver<codex_protocol::protocol::Event>,
) {
    let mut provider =
        codex_model_provider_info::built_in_model_providers(/*openai_base_url*/ None)["openai"]
            .clone();
    provider.base_url = Some(format!("{server_uri}/v1"));
    provider.supports_websockets = false;
    crate::session::tests::make_session_and_context_with_auth_and_config_and_rx(
        CodexAuth::create_dummy_chatgpt_auth_for_testing(),
        Vec::new(),
        move |config| {
            config.model = Some("gpt-5.2".to_string());
            config.model_context_window = Some(100_000);
            config.model_auto_compact_token_limit = Some(90_000);
            config.model_provider = provider;
            config.chatgpt_base_url = server_uri;
            config.features.enable(Feature::AsyncCompaction).unwrap();
            config.features.disable(Feature::TokenBudget).unwrap();
        },
    )
    .await
}

#[tokio::test]
async fn exact_stop_holds_while_speculative_compaction_is_pending() {
    let (release, gate) = oneshot::channel();
    let (server, _completions) = start_streaming_sse_server(vec![vec![
        StreamingSseChunk {
            gate: None,
            body: responses::sse(vec![responses::ev_response_created("compaction-held")]),
        },
        StreamingSseChunk {
            gate: Some(gate),
            body: responses::sse(vec![
                serde_json::json!({"type":"response.output_item.done", "item": {
                    "type":"compaction", "encrypted_content":"prepared-summary"
                }}),
                responses::ev_completed("compaction-held"),
            ]),
        },
    ]])
    .await;
    let (sess, turn, events) = compaction_session(server.uri().to_string()).await;
    sess.spawn_task(
        Arc::clone(&turn),
        Vec::new(),
        HeldStepTask {
            kind: TaskKind::Regular,
            finish: Arc::new(tokio::sync::Notify::new()),
        },
    )
    .await;
    let cancellation = CancellationToken::new();
    let step = sess
        .capture_step_context(Arc::clone(&turn), &cancellation)
        .await
        .unwrap();
    sess.replace_history(
        vec![
            serde_json::from_value(serde_json::json!({
                "type": "message", "role": "user",
                "content": [{"type": "input_text", "text": "original turn"}]
            }))
            .unwrap(),
        ],
        /*reference_context_item*/ None,
    )
    .await;
    sess.update_token_usage_info(
        &turn,
        Some(&TokenUsage {
            input_tokens: 70_000,
            total_tokens: 70_000,
            ..Default::default()
        }),
    )
    .await
    .unwrap();
    let status = super::super::context_window::context_window_token_status(&sess, &turn).await;
    maybe_start(
        &sess,
        &step,
        &Arc::new(WorldState::default()),
        CompactionPhase::MidTurn,
        &cancellation,
        &status,
        (
            Some("routing".into()),
            sess.services.model_client.auth_owner_generation(),
        ),
    )
    .await;
    let deadline = Duration::from_secs(10);
    timeout(deadline, server.wait_for_request_count(1))
        .await
        .unwrap();

    let (reply, accepted) = oneshot::channel();
    sess.interrupt_turn_if_no_pending_input(&turn.sub_id, /*require_unsteered*/ true, reply)
        .await;
    assert!(!accepted.await.unwrap());
    assert!(sess.active_turn.lock().await.is_some());
    sess.abort_all_tasks(codex_protocol::protocol::TurnAbortReason::Interrupted)
        .await;
    timeout(deadline, async {
        loop {
            let event = events.recv().await.unwrap();
            assert!(!matches!(event.msg, EventMsg::ExactStopTaskJoined));
            if matches!(event.msg, EventMsg::TurnAborted(_)) {
                assert_eq!(event.id, turn.sub_id);
                break;
            }
        }
    })
    .await
    .unwrap();
    while let Ok(event) = events.try_recv() {
        assert!(!matches!(event.msg, EventMsg::ExactStopTaskJoined));
    }
    release.send(()).unwrap();
    timeout(deadline, wait_for_background_compaction_for_test(&sess))
        .await
        .unwrap();
    assert_eq!(server.requests().await.len(), 1);
    shutdown(&sess).await;
    server.shutdown().await;
}
