use super::*;

#[test]
fn websocket_message_too_large_falls_back_without_retry() {
    let err = CodexErr::Stream(
        "Responses websocket message exceeded the server size limit. Retrying over HTTPS transport."
            .to_string(),
        Some(Duration::ZERO),
    );

    assert!(should_fallback_to_http_without_retry(&err));
}

#[test]
fn ordinary_stream_disconnect_uses_retry_budget() {
    let err = CodexErr::Stream(
        "websocket closed by server before response.completed".to_string(),
        None,
    );

    assert!(!should_fallback_to_http_without_retry(&err));
}
