use super::*;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn omits_unset_reasoning_from_responses_requests() {
    let request = ResponsesApiRequest {
        model: "gpt-test".to_string(),
        instructions: "Say hi".to_string(),
        input: Vec::new(),
        tools: Some(Vec::new()),
        tool_choice: "auto".to_string(),
        parallel_tool_calls: false,
        reasoning: None,
        store: false,
        stream: true,
        include: Vec::new(),
        service_tier: None,
        prompt_cache_key: None,
        text: None,
        client_metadata: None,
    };

    let expected_http_request = json!({
        "model": "gpt-test",
        "instructions": "Say hi",
        "input": [],
        "tools": [],
        "tool_choice": "auto",
        "parallel_tool_calls": false,
        "store": false,
        "stream": true,
        "include": [],
    });
    assert_eq!(
        serde_json::to_value(&request).expect("serialize responses request"),
        expected_http_request
    );

    let expected_ws_request = json!({
        "type": "response.create",
        "model": "gpt-test",
        "instructions": "Say hi",
        "input": [],
        "tools": [],
        "tool_choice": "auto",
        "parallel_tool_calls": false,
        "store": false,
        "stream": true,
        "include": [],
    });
    assert_eq!(
        serde_json::to_value(ResponsesWsRequest::ResponseCreate(
            ResponseCreateWsRequest::from(&request)
        ))
        .expect("serialize websocket request"),
        expected_ws_request
    );
}
