use super::DynamicToolFunctionSpec;
use super::DynamicToolNamespaceSpec;
use super::DynamicToolNamespaceTool;
use super::DynamicToolSpec;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn dynamic_tool_function_defaults_missing_input_schema() {
    let spec = serde_json::from_value::<DynamicToolSpec>(json!({
        "type": "function",
        "name": "load_workspace_dependencies",
        "description": "Load workspace dependencies"
    }))
    .expect("dynamic tool should deserialize");

    assert_eq!(
        spec,
        DynamicToolSpec::Function(DynamicToolFunctionSpec {
            name: "load_workspace_dependencies".to_string(),
            description: "Load workspace dependencies".to_string(),
            input_schema: json!({"type": "object", "properties": {}}),
            defer_loading: false,
        })
    );
}

#[test]
fn namespace_dynamic_tool_defaults_missing_input_schema() {
    let spec = serde_json::from_value::<DynamicToolSpec>(json!({
        "type": "namespace",
        "name": "codex_app",
        "description": "Codex app tools",
        "tools": [
            {
                "type": "function",
                "name": "fork_thread",
                "description": "Fork a thread"
            }
        ]
    }))
    .expect("namespace dynamic tool should deserialize");

    assert_eq!(
        spec,
        DynamicToolSpec::Namespace(DynamicToolNamespaceSpec {
            name: "codex_app".to_string(),
            description: "Codex app tools".to_string(),
            tools: vec![DynamicToolNamespaceTool::Function(
                DynamicToolFunctionSpec {
                    name: "fork_thread".to_string(),
                    description: "Fork a thread".to_string(),
                    input_schema: json!({"type": "object", "properties": {}}),
                    defer_loading: false,
                }
            )],
        })
    );
}
