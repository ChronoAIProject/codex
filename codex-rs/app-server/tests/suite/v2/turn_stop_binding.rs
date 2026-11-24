use anyhow::Context;
use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::create_mock_responses_server_repeating_assistant;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadInjectItemsParams;
use codex_app_server_protocol::ThreadInjectItemsResponse;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadUnsubscribeParams;
use codex_app_server_protocol::ThreadUnsubscribeResponse;
use codex_app_server_protocol::ThreadUnsubscribeStatus;
use codex_app_server_protocol::TurnCompletedNotification;
use codex_app_server_protocol::TurnStartBoundParams;
use codex_app_server_protocol::TurnStartBoundResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::TurnSteerParams;
use codex_app_server_protocol::TurnSteerResponse;
use codex_app_server_protocol::TurnStopCapabilityParams;
use codex_app_server_protocol::TurnStopCapabilityResponse;
use codex_app_server_protocol::TurnStopExactResponse;
use codex_app_server_protocol::TurnStopStatus;
use codex_app_server_protocol::TurnStopStatusParams;
use codex_app_server_protocol::TurnStopStatusResponse;
use codex_app_server_protocol::UserInput;
use core_test_support::responses;
use core_test_support::streaming_sse::StreamingSseChunk;
use core_test_support::streaming_sse::start_streaming_sse_server;
use serde_json::json;
use tempfile::TempDir;
use tokio::sync::oneshot;
use tokio::time::Duration;
use tokio::time::timeout;

const ENABLE_ENV: &str = "CHRONO_CODEX_EXACT_TURN_STOP_V1";
const TEST_TIMEOUT: Duration = Duration::from_secs(15);

#[tokio::test]
async fn bound_turn_preserves_canonical_completion_and_rejects_replay() -> Result<()> {
    let model = create_mock_responses_server_repeating_assistant("Ready").await;
    let home = TempDir::new()?;
    MockResponsesConfig::new(&model.uri()).write(home.path())?;
    let mut server = TestAppServer::builder()
        .with_codex_home(home.path())
        .with_env_overrides(&[(ENABLE_ENV, Some("1"))])
        .build_initialized()
        .await?;
    let thread_id = server
        .start_thread(ThreadStartParams {
            model: Some("mock-model".into()),
            ..Default::default()
        })
        .await?
        .thread
        .id;
    let capability: TurnStopCapabilityResponse = server
        .request(|request_id| ClientRequest::TurnStopCapability {
            request_id,
            params: TurnStopCapabilityParams {
                thread_id: thread_id.clone(),
            },
        })
        .await?;
    assert_eq!(capability.binding_version, Some(1));
    let listener_incarnation = capability.listener_incarnation.context("live listener")?;
    let start = TurnStartBoundParams {
        turn: TurnStartParams {
            thread_id: thread_id.clone(),
            input: vec![UserInput::Text {
                text: "Reply with Ready".into(),
                text_elements: vec![],
            }],
            ..Default::default()
        },
        listener_incarnation: listener_incarnation.clone(),
        dispatch_sequence: 1,
        dispatch_key: "accepted-dispatch".into(),
    };
    let bound: TurnStartBoundResponse = server
        .request(|request_id| ClientRequest::TurnStartBound {
            request_id,
            params: start.clone(),
        })
        .await?;
    let completed: TurnCompletedNotification = server.read_notification("turn/completed").await?;
    assert_eq!(completed.turn.id, bound.turn.id);
    assert_eq!(completed.turn.status, TurnStatus::Completed);

    for (key, expected) in [
        ("incorrect-key", TurnStopStatus::Hold),
        ("accepted-dispatch", TurnStopStatus::Canonical),
        ("accepted-dispatch", TurnStopStatus::Canonical),
    ] {
        let result: TurnStopStatusResponse = server
            .request(|request_id| ClientRequest::TurnStopStatus {
                request_id,
                params: TurnStopStatusParams {
                    thread_id: thread_id.clone(),
                    turn_id: bound.turn.id.clone(),
                    listener_incarnation: listener_incarnation.clone(),
                    dispatch_sequence: 1,
                    dispatch_key: key.into(),
                },
            })
            .await?;
        assert_eq!(result.status, expected);
    }

    let replay = server
        .send_raw_request("turn/startBound", Some(serde_json::to_value(start)?))
        .await?;
    let error = server
        .read_stream_until_error_message(RequestId::Integer(replay))
        .await?;
    assert!(error.error.message.contains("repeated exact turn binding"));
    Ok(())
}

#[tokio::test]
async fn binding_is_disabled_by_default() -> Result<()> {
    let mut server = TestAppServer::builder()
        .with_env_overrides(&[(ENABLE_ENV, None)])
        .build_initialized()
        .await?;
    let thread_id = server
        .start_thread(ThreadStartParams::default())
        .await?
        .thread
        .id;
    let capability: TurnStopCapabilityResponse = server
        .request(|request_id| ClientRequest::TurnStopCapability {
            request_id,
            params: TurnStopCapabilityParams {
                thread_id: thread_id.clone(),
            },
        })
        .await?;
    assert_eq!(capability.binding_version, None);
    let attempt = server
        .send_raw_request(
            "turn/startBound",
            Some(json!({
                "turn": { "threadId": thread_id, "input": [] },
                "listenerIncarnation": "unknown",
                "dispatchSequence": 1,
                "dispatchKey": "key",
            })),
        )
        .await?;
    let error = server
        .read_stream_until_error_message(RequestId::Integer(attempt))
        .await?;
    assert_eq!(error.error.message, "exact turn binding is disabled");
    Ok(())
}

#[tokio::test]
async fn exact_stop_preserves_ordinary_cancellation_across_replay() -> Result<()> {
    let (_release, gate) = oneshot::channel();
    let (model, _completions) = start_streaming_sse_server(vec![vec![
        StreamingSseChunk {
            gate: None,
            body: responses::sse(vec![responses::ev_response_created("held-response")]),
        },
        StreamingSseChunk {
            gate: Some(gate),
            body: responses::sse(vec![responses::ev_completed("held-response")]),
        },
    ]])
    .await;
    let (mut server, mut target, _home) = start_bound_turn(model.uri()).await?;
    timeout(TEST_TIMEOUT, model.wait_for_request_count(1)).await?;
    let refused_start = server
        .send_raw_request(
            "turn/startBound",
            Some(json!({
                "turn": { "threadId": target.thread_id, "input": [
                    { "type": "text", "text": "refused successor", "textElements": [] }
                ] },
                "listenerIncarnation": target.listener_incarnation,
                "dispatchSequence": 2,
                "dispatchKey": "successor-dispatch",
            })),
        )
        .await?;
    let refused = server
        .read_stream_until_error_message(RequestId::Integer(refused_start))
        .await?;
    assert!(refused.error.message.contains("NotIdle"), "{refused:?}");
    target.dispatch_key = "wrong".into();
    let wrong: TurnStopExactResponse = server
        .request(|request_id| ClientRequest::TurnStopExact {
            request_id,
            params: target.clone(),
        })
        .await?;
    assert_eq!(wrong.status, TurnStopStatus::Hold);
    target.dispatch_key = "accepted-dispatch".into();
    let before: TurnStopStatusResponse = server
        .request(|request_id| ClientRequest::TurnStopStatus {
            request_id,
            params: target.clone(),
        })
        .await?;
    assert_eq!(before.status, TurnStopStatus::Pending);

    let _: TurnStopExactResponse = server
        .request(|request_id| ClientRequest::TurnStopExact {
            request_id,
            params: target.clone(),
        })
        .await?;
    let completed: TurnCompletedNotification =
        timeout(TEST_TIMEOUT, server.read_notification("turn/completed")).await??;
    assert_eq!(completed.turn.id, target.turn_id);
    assert_eq!(completed.turn.status, TurnStatus::Interrupted);
    let replay: TurnStopExactResponse = server
        .request(|request_id| ClientRequest::TurnStopExact {
            request_id,
            params: target,
        })
        .await?;
    assert_eq!(replay.status, TurnStopStatus::Canonical);
    assert_eq!(model.requests().await.len(), 1);
    Ok(())
}

#[test_case::test_case(false; "steered")]
#[test_case::test_case(true; "client_injected")]
#[tokio::test]
async fn exact_stop_holds_after_core_consumes_later_input(inject: bool) -> Result<()> {
    const MARKER: &str = "independently accepted input";
    let (release_initial, initial_gate) = oneshot::channel();
    let (release_steered, steered_gate) = oneshot::channel();
    let (model, _completions) = start_streaming_sse_server(vec![
        vec![
            StreamingSseChunk {
                gate: None,
                body: responses::sse(vec![responses::ev_response_created("initial")]),
            },
            StreamingSseChunk {
                gate: Some(initial_gate),
                body: responses::sse(vec![responses::ev_completed("initial")]),
            },
        ],
        vec![
            StreamingSseChunk {
                gate: None,
                body: responses::sse(vec![responses::ev_response_created("steered")]),
            },
            StreamingSseChunk {
                gate: Some(steered_gate),
                body: responses::sse(vec![
                    responses::ev_assistant_message("message", "steered result"),
                    responses::ev_completed("steered"),
                ]),
            },
        ],
    ])
    .await;
    let (mut server, target, _home) = start_bound_turn(model.uri()).await?;
    timeout(TEST_TIMEOUT, model.wait_for_request_count(1)).await?;
    if inject {
        let _: ThreadInjectItemsResponse = server
            .request(|request_id| ClientRequest::ThreadInjectItems {
                request_id,
                params: ThreadInjectItemsParams {
                    thread_id: target.thread_id.clone(),
                    items: vec![json!({
                        "type": "message", "role": "user",
                        "content": [{ "type": "input_text", "text": MARKER }],
                    })],
                },
            })
            .await?;
    } else {
        let steered: TurnSteerResponse = server
            .request(|request_id| ClientRequest::TurnSteer {
                request_id,
                params: TurnSteerParams {
                    thread_id: target.thread_id.clone(),
                    expected_turn_id: target.turn_id.clone(),
                    input: vec![UserInput::Text {
                        text: MARKER.into(),
                        text_elements: vec![],
                    }],
                    ..Default::default()
                },
            })
            .await?;
        assert_eq!(steered.turn_id, target.turn_id);
    }
    release_initial.send(()).expect("stream is waiting");
    timeout(TEST_TIMEOUT, model.wait_for_request_count(2)).await?;
    let requests = model.requests().await;
    let request: serde_json::Value = serde_json::from_slice(&requests[1])?;
    assert!(request["input"].to_string().contains(MARKER));
    let stop: TurnStopExactResponse = server
        .request(|request_id| ClientRequest::TurnStopExact {
            request_id,
            params: target.clone(),
        })
        .await?;
    assert_eq!(stop.status, TurnStopStatus::Hold);
    release_steered.send(()).expect("steered stream is waiting");
    let completed: TurnCompletedNotification =
        timeout(TEST_TIMEOUT, server.read_notification("turn/completed")).await??;
    assert_eq!(completed.turn.id, target.turn_id);
    assert_eq!(completed.turn.status, TurnStatus::Completed);
    assert_eq!(model.requests().await.len(), 2);
    Ok(())
}

async fn start_bound_turn(
    model_uri: &str,
) -> Result<(TestAppServer, TurnStopStatusParams, TempDir)> {
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(model_uri).write(codex_home.path())?;
    let mut server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .with_env_overrides(&[(ENABLE_ENV, Some("1"))])
        .build_initialized()
        .await?;
    let thread_id = server
        .start_thread(ThreadStartParams {
            model: Some("mock-model".into()),
            ..Default::default()
        })
        .await?
        .thread
        .id;
    let capability: TurnStopCapabilityResponse = server
        .request(|request_id| ClientRequest::TurnStopCapability {
            request_id,
            params: TurnStopCapabilityParams {
                thread_id: thread_id.clone(),
            },
        })
        .await?;
    let listener_incarnation = capability.listener_incarnation.context("live listener")?;
    let dispatch_key = "accepted-dispatch".to_string();
    let bound: TurnStartBoundResponse = server
        .request(|request_id| ClientRequest::TurnStartBound {
            request_id,
            params: TurnStartBoundParams {
                turn: TurnStartParams {
                    thread_id: thread_id.clone(),
                    input: vec![UserInput::Text {
                        text: "Wait for the response".into(),
                        text_elements: vec![],
                    }],
                    ..Default::default()
                },
                listener_incarnation: listener_incarnation.clone(),
                dispatch_sequence: 1,
                dispatch_key: dispatch_key.clone(),
            },
        })
        .await?;
    Ok((
        server,
        TurnStopStatusParams {
            thread_id,
            turn_id: bound.turn.id,
            listener_incarnation,
            dispatch_sequence: 1,
            dispatch_key,
        },
        codex_home,
    ))
}

#[tokio::test]
async fn subscriber_gap_disqualifies_old_incarnation_without_stopping_turn() -> Result<()> {
    let (release, gate) = oneshot::channel();
    let (model, _completions) = start_streaming_sse_server(vec![vec![
        StreamingSseChunk {
            gate: None,
            body: responses::sse(vec![responses::ev_response_created("held-response")]),
        },
        StreamingSseChunk {
            gate: Some(gate),
            body: responses::sse(vec![
                responses::ev_assistant_message("result", "completed naturally"),
                responses::ev_completed("held-response"),
            ]),
        },
    ]])
    .await;
    let (mut server, target, _home) = start_bound_turn(model.uri()).await?;
    timeout(TEST_TIMEOUT, model.wait_for_request_count(1)).await?;
    let unsubscribed: ThreadUnsubscribeResponse = server
        .request(|request_id| ClientRequest::ThreadUnsubscribe {
            request_id,
            params: ThreadUnsubscribeParams {
                thread_id: target.thread_id.clone(),
            },
        })
        .await?;
    assert_eq!(unsubscribed.status, ThreadUnsubscribeStatus::Unsubscribed);
    let _: ThreadResumeResponse = server
        .request(|request_id| ClientRequest::ThreadResume {
            request_id,
            params: ThreadResumeParams {
                thread_id: target.thread_id.clone(),
                ..Default::default()
            },
        })
        .await?;
    let capability: TurnStopCapabilityResponse = server
        .request(|request_id| ClientRequest::TurnStopCapability {
            request_id,
            params: TurnStopCapabilityParams {
                thread_id: target.thread_id.clone(),
            },
        })
        .await?;
    let fresh = capability
        .listener_incarnation
        .context("replacement listener")?;
    assert_ne!(fresh, target.listener_incarnation);
    let stop: TurnStopExactResponse = server
        .request(|request_id| ClientRequest::TurnStopExact {
            request_id,
            params: target.clone(),
        })
        .await?;
    assert_eq!(stop.status, TurnStopStatus::Hold);
    release.send(()).expect("stream is waiting");
    let completed: TurnCompletedNotification =
        timeout(TEST_TIMEOUT, server.read_notification("turn/completed")).await??;
    assert_eq!(completed.turn.id, target.turn_id);
    assert_eq!(completed.turn.status, TurnStatus::Completed);
    let stale_start = server
        .send_raw_request(
            "turn/startBound",
            Some(json!({
                "turn": { "threadId": target.thread_id, "input": [
                    { "type": "text", "text": "stale subscriber", "textElements": [] }
                ] },
                "listenerIncarnation": target.listener_incarnation,
                "dispatchSequence": 2,
                "dispatchKey": "stale-subscriber-dispatch",
            })),
        )
        .await?;
    let error = server
        .read_stream_until_error_message(RequestId::Integer(stale_start))
        .await?;
    assert_eq!(
        error.error.message,
        "invalid or repeated exact turn binding"
    );
    assert_eq!(model.requests().await.len(), 1);
    Ok(())
}
