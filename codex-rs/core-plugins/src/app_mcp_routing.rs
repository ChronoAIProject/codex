use codex_plugin::AppDeclaration;
use codex_protocol::auth::AuthMode;
use std::collections::HashMap;
use std::collections::HashSet;

pub fn apps_route_available(auth_mode: Option<AuthMode>) -> bool {
    auth_mode.is_some_and(AuthMode::uses_codex_backend)
}

pub(crate) fn apply_app_mcp_routing_policy<M>(
    apps: &mut Vec<AppDeclaration>,
    mcp_servers: &mut HashMap<String, M>,
    auth_mode: Option<AuthMode>,
    plugin_active: bool,
) {
    if !apps_route_available(auth_mode) {
        apps.clear();
        return;
    }

    if plugin_active && !apps.is_empty() {
        let app_declaration_names = apps
            .iter()
            .filter(|app| !is_app_directory_connector_id(&app.connector_id.0))
            .map(|app| app.name.as_str())
            .collect::<HashSet<_>>();
        mcp_servers.retain(|name, _| !app_declaration_names.contains(name.as_str()));
    }
}

fn is_app_directory_connector_id(connector_id: &str) -> bool {
    connector_id.starts_with("asdk_app_")
}

#[cfg(test)]
#[path = "app_mcp_routing_tests.rs"]
mod tests;
