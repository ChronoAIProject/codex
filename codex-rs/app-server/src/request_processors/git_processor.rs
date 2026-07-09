use super::*;
use std::process::Stdio;
use tokio::process::Command;

#[derive(Clone)]
pub(crate) struct GitRequestProcessor;

impl GitRequestProcessor {
    pub(crate) fn new() -> Self {
        Self
    }

    pub(crate) async fn git_diff_to_remote(
        &self,
        params: GitDiffToRemoteParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.git_diff_to_origin(params.cwd, params.sha)
            .await
            .map(|response| Some(response.into()))
    }

    async fn git_diff_to_origin(
        &self,
        cwd: PathBuf,
        sha: Option<codex_app_server_protocol::GitSha>,
    ) -> Result<GitDiffToRemoteResponse, JSONRPCErrorError> {
        if let Some(sha) = sha {
            return diff_against_sha(&cwd, sha).await.ok_or_else(|| {
                invalid_request(format!("failed to compute git diff for cwd: {cwd:?}"))
            });
        }

        git_diff_to_remote(&cwd).await.map_or_else(
            || {
                Err(invalid_request(format!(
                    "failed to compute git diff to remote for cwd: {cwd:?}"
                )))
            },
            |value| {
                Ok(GitDiffToRemoteResponse {
                    sha: value.sha,
                    diff: value.diff,
                })
            },
        )
    }
}

async fn diff_against_sha(
    cwd: &Path,
    sha: codex_app_server_protocol::GitSha,
) -> Option<GitDiffToRemoteResponse> {
    let output = Command::new("git")
        .args(["diff", "--no-textconv", "--no-ext-diff", &sha.0])
        .current_dir(cwd)
        .stdin(Stdio::null())
        .kill_on_drop(true)
        .output()
        .await
        .ok()?;

    let exit_ok = output
        .status
        .code()
        .is_some_and(|code| code == 0 || code == 1);
    if !exit_ok {
        return None;
    }

    Some(GitDiffToRemoteResponse {
        sha,
        diff: String::from_utf8(output.stdout).ok()?,
    })
}

#[cfg(test)]
#[path = "git_processor_tests.rs"]
mod tests;
