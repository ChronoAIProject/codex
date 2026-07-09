use super::*;
use codex_app_server_protocol::GitSha;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::process::Command;

#[tokio::test]
async fn git_diff_to_remote_uses_requested_sha_as_diff_base() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let repo_path = temp_dir.path().join("repo");
    std::fs::create_dir(&repo_path).expect("create repo dir");

    run_git(&repo_path, &["init"]).await;
    run_git(&repo_path, &["config", "user.name", "Test User"]).await;
    run_git(&repo_path, &["config", "user.email", "test@example.com"]).await;

    std::fs::write(repo_path.join("reviewed.txt"), "before\n").expect("write initial file");
    run_git(&repo_path, &["add", "."]).await;
    run_git(&repo_path, &["commit", "-m", "initial"]).await;
    let review_base_sha = git_stdout(&repo_path, &["rev-parse", "HEAD"]).await;

    std::fs::write(repo_path.join("reviewed.txt"), "reviewed\n").expect("write reviewed change");
    run_git(&repo_path, &["add", "."]).await;
    run_git(&repo_path, &["commit", "-m", "reviewed change"]).await;

    std::fs::write(repo_path.join("reviewed.txt"), "latest\n").expect("write later change");
    let response = GitRequestProcessor::new()
        .git_diff_to_origin(repo_path, Some(GitSha::new(&review_base_sha)))
        .await
        .expect("diff against requested sha");

    assert_eq!(response.sha, GitSha::new(&review_base_sha));
    assert!(response.diff.contains("+latest"));
    assert!(response.diff.contains("-before"));
}

async fn run_git(cwd: &Path, args: &[&str]) {
    let output = Command::new("git")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .args(args)
        .current_dir(cwd)
        .output()
        .await
        .expect("run git");
    assert!(output.status.success(), "git {args:?} failed: {output:?}");
}

async fn git_stdout(cwd: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .args(args)
        .current_dir(cwd)
        .output()
        .await
        .expect("run git");
    assert!(output.status.success(), "git {args:?} failed: {output:?}");
    String::from_utf8(output.stdout)
        .expect("git stdout is utf8")
        .trim()
        .to_string()
}
