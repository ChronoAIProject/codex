use codex_client::build_reqwest_client_with_custom_ca;
use reqwest::header::CONTENT_TYPE;
use reqwest::header::HeaderMap;
use std::collections::HashMap;
use tracing::info;
use tracing::warn;

#[derive(Debug, Clone, serde::Deserialize)]
struct CodeEnvironment {
    id: String,
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    is_pinned: Option<bool>,
    #[serde(default)]
    task_count: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct AutodetectSelection {
    pub id: String,
    pub label: Option<String>,
}

pub async fn autodetect_environment_id(
    base_url: &str,
    headers: &HeaderMap,
    desired_label: Option<String>,
) -> anyhow::Result<AutodetectSelection> {
    // 1) Try repo-specific environments based on local git origins.
    let origins = get_git_origins();
    crate::append_error_log(format!("env: git origins: {origins:?}"));
    let mut by_repo_envs: Vec<CodeEnvironment> = Vec::new();
    for origin in &origins {
        if let Some(remote) = parse_repo_remote(origin) {
            let by_repo_path = remote.by_repo_path();
            let url = if base_url.contains("/backend-api") {
                format!("{base_url}/wham/environments/by-repo/{by_repo_path}")
            } else {
                format!("{base_url}/api/codex/environments/by-repo/{by_repo_path}")
            };
            crate::append_error_log(format!("env: GET {url}"));
            match get_json::<Vec<CodeEnvironment>>(&url, headers).await {
                Ok(mut list) => {
                    crate::append_error_log(format!(
                        "env: by-repo returned {} env(s) for {}",
                        list.len(),
                        remote.display_name(),
                    ));
                    by_repo_envs.append(&mut list);
                }
                Err(e) => crate::append_error_log(format!(
                    "env: by-repo fetch failed for {}: {e}",
                    remote.display_name()
                )),
            }
        }
    }
    if let Some(env) = pick_environment_row(&by_repo_envs, desired_label.as_deref()) {
        return Ok(AutodetectSelection {
            id: env.id.clone(),
            label: env.label.as_deref().map(str::to_owned),
        });
    }

    // 2) Fallback to the full list
    let list_url = if base_url.contains("/backend-api") {
        format!("{base_url}/wham/environments")
    } else {
        format!("{base_url}/api/codex/environments")
    };
    crate::append_error_log(format!("env: GET {list_url}"));
    // Fetch and log the full environments JSON for debugging
    let http = build_reqwest_client_with_custom_ca(reqwest::Client::builder())?;
    let res = http.get(&list_url).headers(headers.clone()).send().await?;
    let status = res.status();
    let ct = res
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let body = res.text().await.unwrap_or_default();
    crate::append_error_log(format!("env: status={status} content-type={ct}"));
    match serde_json::from_str::<serde_json::Value>(&body) {
        Ok(v) => {
            let pretty = serde_json::to_string_pretty(&v).unwrap_or(body.clone());
            crate::append_error_log(format!("env: /environments JSON (pretty):\n{pretty}"));
        }
        Err(_) => crate::append_error_log(format!("env: /environments (raw):\n{body}")),
    }
    if !status.is_success() {
        anyhow::bail!("GET {list_url} failed: {status}; content-type={ct}; body={body}");
    }
    let all_envs: Vec<CodeEnvironment> = serde_json::from_str(&body).map_err(|e| {
        anyhow::anyhow!("Decode error for {list_url}: {e}; content-type={ct}; body={body}")
    })?;
    if let Some(env) = pick_environment_row(&all_envs, desired_label.as_deref()) {
        return Ok(AutodetectSelection {
            id: env.id.clone(),
            label: env.label.as_deref().map(str::to_owned),
        });
    }
    anyhow::bail!("no environments available")
}

fn pick_environment_row(
    envs: &[CodeEnvironment],
    desired_label: Option<&str>,
) -> Option<CodeEnvironment> {
    if envs.is_empty() {
        return None;
    }
    if let Some(label) = desired_label {
        let lc = label.to_lowercase();
        if let Some(e) = envs
            .iter()
            .find(|e| e.label.as_deref().unwrap_or("").to_lowercase() == lc)
        {
            crate::append_error_log(format!("env: matched by label: {label} -> {}", e.id));
            return Some(e.clone());
        }
    }
    if envs.len() == 1 {
        crate::append_error_log("env: single environment available; selecting it");
        return Some(envs[0].clone());
    }
    if let Some(e) = envs.iter().find(|e| e.is_pinned.unwrap_or(false)) {
        crate::append_error_log(format!("env: selecting pinned environment: {}", e.id));
        return Some(e.clone());
    }
    // Highest task_count as heuristic
    if let Some(e) = envs
        .iter()
        .max_by_key(|e| e.task_count.unwrap_or(0))
        .or_else(|| envs.first())
    {
        crate::append_error_log(format!("env: selecting by task_count/first: {}", e.id));
        return Some(e.clone());
    }
    None
}

async fn get_json<T: serde::de::DeserializeOwned>(
    url: &str,
    headers: &HeaderMap,
) -> anyhow::Result<T> {
    let http = build_reqwest_client_with_custom_ca(reqwest::Client::builder())?;
    let res = http.get(url).headers(headers.clone()).send().await?;
    let status = res.status();
    let ct = res
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let body = res.text().await.unwrap_or_default();
    crate::append_error_log(format!("env: status={status} content-type={ct}"));
    if !status.is_success() {
        anyhow::bail!("GET {url} failed: {status}; content-type={ct}; body={body}");
    }
    let parsed = serde_json::from_str::<T>(&body).map_err(|e| {
        anyhow::anyhow!("Decode error for {url}: {e}; content-type={ct}; body={body}")
    })?;
    Ok(parsed)
}

fn get_git_origins() -> Vec<String> {
    // Prefer: git config --get-regexp remote\..*\.url
    let out = std::process::Command::new("git")
        .args(["config", "--get-regexp", "remote\\..*\\.url"])
        .output();
    if let Ok(ok) = out
        && ok.status.success()
    {
        let s = String::from_utf8_lossy(&ok.stdout);
        let mut urls = Vec::new();
        for line in s.lines() {
            if let Some((_, url)) = line.split_once(' ') {
                urls.push(url.trim().to_string());
            }
        }
        if !urls.is_empty() {
            return uniq(urls);
        }
    }
    // Fallback: git remote -v
    let out = std::process::Command::new("git")
        .args(["remote", "-v"])
        .output();
    if let Ok(ok) = out
        && ok.status.success()
    {
        let s = String::from_utf8_lossy(&ok.stdout);
        let mut urls = Vec::new();
        for line in s.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                urls.push(parts[1].to_string());
            }
        }
        if !urls.is_empty() {
            return uniq(urls);
        }
    }
    Vec::new()
}

fn uniq(mut v: Vec<String>) -> Vec<String> {
    v.sort();
    v.dedup();
    v
}

#[derive(Debug, PartialEq, Eq)]
struct RepoRemote {
    provider: &'static str,
    owner: String,
    repo: String,
}

impl RepoRemote {
    fn by_repo_path(&self) -> String {
        format!(
            "{}/{}/{}",
            self.provider,
            escape_path_segment(&self.owner),
            escape_path_segment(&self.repo)
        )
    }

    fn display_name(&self) -> String {
        format!("{}/{}", self.owner, self.repo)
    }
}

fn escape_path_segment(segment: &str) -> String {
    let mut escaped = String::new();
    for &byte in segment.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                escaped.push(byte as char);
            }
            _ => escaped.push_str(&format!("%{byte:02X}")),
        }
    }
    escaped
}

fn unescape_path_segment(segment: &str) -> Option<String> {
    let bytes = segment.as_bytes();
    let mut unescaped = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if i + 2 >= bytes.len() {
                return None;
            }
            let hi = hex_value(bytes[i + 1])?;
            let lo = hex_value(bytes[i + 2])?;
            unescaped.push(hi << 4 | lo);
            i += 3;
        } else {
            unescaped.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(unescaped).ok()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn parse_repo_remote(url: &str) -> Option<RepoRemote> {
    // Normalize common prefixes and handle multiple SSH/HTTPS variants.
    let mut s = url.trim().to_string();
    // Drop protocol scheme for ssh URLs
    if let Some(rest) = s.strip_prefix("ssh://") {
        s = rest.to_string();
    }
    // Accept any user before @github.com (e.g., git@, org-123@)
    if let Some(idx) = s.find("@github.com:") {
        let rest = &s[idx + "@github.com:".len()..];
        let rest = rest.trim_start_matches('/').trim_end_matches(".git");
        let mut parts = rest.splitn(2, '/');
        let owner = parts.next()?.to_string();
        let repo = parts.next()?.to_string();
        crate::append_error_log(format!("env: parsed SSH GitHub origin => {owner}/{repo}"));
        return Some(RepoRemote {
            provider: "github",
            owner,
            repo,
        });
    }
    // HTTPS or git protocol
    for prefix in [
        "https://github.com/",
        "http://github.com/",
        "git://github.com/",
        "github.com/",
    ] {
        if let Some(rest) = s.strip_prefix(prefix) {
            let rest = rest.trim_start_matches('/').trim_end_matches(".git");
            let mut parts = rest.splitn(2, '/');
            let owner = parts.next()?.to_string();
            let repo = parts.next()?.to_string();
            crate::append_error_log(format!("env: parsed HTTP GitHub origin => {owner}/{repo}"));
            return Some(RepoRemote {
                provider: "github",
                owner,
                repo,
            });
        }
    }
    for prefix in [
        "https://dev.azure.com/",
        "http://dev.azure.com/",
        "dev.azure.com/",
    ] {
        if let Some(rest) = s.strip_prefix(prefix) {
            let rest = rest.trim_start_matches('/').trim_end_matches(".git");
            let mut parts = rest.splitn(4, '/');
            let organization = parts.next()?;
            let project = unescape_path_segment(parts.next()?)?;
            if parts.next()? != "_git" {
                return None;
            }
            let repo = unescape_path_segment(parts.next()?)?;
            let owner = format!("{organization}/{project}");
            crate::append_error_log(format!("env: parsed Azure Repos origin => {owner}/{repo}"));
            return Some(RepoRemote {
                provider: "azure_devops",
                owner,
                repo,
            });
        }
    }
    for host in ["@ssh.dev.azure.com:", "@vs-ssh.visualstudio.com:"] {
        let Some(idx) = s.find(host) else {
            continue;
        };
        let rest = &s[idx + host.len()..];
        let rest = rest.trim_start_matches('/').trim_end_matches(".git");
        let mut parts = rest.splitn(4, '/');
        if parts.next()? != "v3" {
            return None;
        }
        let organization = parts.next()?;
        let project = unescape_path_segment(parts.next()?)?;
        let repo = unescape_path_segment(parts.next()?)?;
        let owner = format!("{organization}/{project}");
        crate::append_error_log(format!(
            "env: parsed Azure Repos SSH origin => {owner}/{repo}"
        ));
        return Some(RepoRemote {
            provider: "azure_devops",
            owner,
            repo,
        });
    }
    None
}

/// List environments for the current repo(s) with a fallback to the global list.
/// Returns a de-duplicated, sorted set suitable for the TUI modal.
pub async fn list_environments(
    base_url: &str,
    headers: &HeaderMap,
) -> anyhow::Result<Vec<crate::app::EnvironmentRow>> {
    let mut map: HashMap<String, crate::app::EnvironmentRow> = HashMap::new();

    // 1) By-repo lookup for each parsed origin
    let origins = get_git_origins();
    for origin in &origins {
        if let Some(remote) = parse_repo_remote(origin) {
            let by_repo_path = remote.by_repo_path();
            let display_name = remote.display_name();
            let url = if base_url.contains("/backend-api") {
                format!("{base_url}/wham/environments/by-repo/{by_repo_path}")
            } else {
                format!("{base_url}/api/codex/environments/by-repo/{by_repo_path}")
            };
            match get_json::<Vec<CodeEnvironment>>(&url, headers).await {
                Ok(list) => {
                    info!("env_tui: by-repo {} -> {} envs", display_name, list.len());
                    for e in list {
                        let entry =
                            map.entry(e.id.clone())
                                .or_insert_with(|| crate::app::EnvironmentRow {
                                    id: e.id.clone(),
                                    label: e.label.clone(),
                                    is_pinned: e.is_pinned.unwrap_or(false),
                                    repo_hints: Some(display_name.clone()),
                                });
                        // Merge: keep label if present, or use new; accumulate pinned flag
                        if entry.label.is_none() {
                            entry.label = e.label.clone();
                        }
                        entry.is_pinned = entry.is_pinned || e.is_pinned.unwrap_or(false);
                        if entry.repo_hints.is_none() {
                            entry.repo_hints = Some(display_name.clone());
                        }
                    }
                }
                Err(e) => {
                    warn!("env_tui: by-repo fetch failed for {}: {}", display_name, e);
                }
            }
        }
    }

    // 2) Fallback to the full list; on error return what we have if any.
    let list_url = if base_url.contains("/backend-api") {
        format!("{base_url}/wham/environments")
    } else {
        format!("{base_url}/api/codex/environments")
    };
    match get_json::<Vec<CodeEnvironment>>(&list_url, headers).await {
        Ok(list) => {
            info!("env_tui: global list -> {} envs", list.len());
            for e in list {
                let entry = map
                    .entry(e.id.clone())
                    .or_insert_with(|| crate::app::EnvironmentRow {
                        id: e.id.clone(),
                        label: e.label.clone(),
                        is_pinned: e.is_pinned.unwrap_or(false),
                        repo_hints: None,
                    });
                if entry.label.is_none() {
                    entry.label = e.label.clone();
                }
                entry.is_pinned = entry.is_pinned || e.is_pinned.unwrap_or(false);
            }
        }
        Err(e) => {
            if map.is_empty() {
                return Err(e);
            } else {
                warn!(
                    "env_tui: global list failed; using by-repo results only: {}",
                    e
                );
            }
        }
    }

    let mut rows: Vec<crate::app::EnvironmentRow> = map.into_values().collect();
    rows.sort_by(|a, b| {
        // pinned first
        let p = b.is_pinned.cmp(&a.is_pinned);
        if p != std::cmp::Ordering::Equal {
            return p;
        }
        // then label (ci), then id
        let al = a.label.as_deref().unwrap_or("").to_lowercase();
        let bl = b.label.as_deref().unwrap_or("").to_lowercase();
        let l = al.cmp(&bl);
        if l != std::cmp::Ordering::Equal {
            return l;
        }
        a.id.cmp(&b.id)
    });
    Ok(rows)
}

#[cfg(test)]
#[path = "env_detect_tests.rs"]
mod tests;
