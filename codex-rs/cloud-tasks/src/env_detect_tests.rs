use super::RepoRemote;
use super::parse_repo_remote;

#[test]
fn parse_repo_remote_supports_github_and_azure_repos() {
    assert_eq!(
        parse_repo_remote("git@github.com:openai/codex.git"),
        Some(RepoRemote {
            provider: "github",
            owner: "openai".to_string(),
            repo: "codex".to_string(),
        })
    );
    assert_eq!(
        parse_repo_remote("https://github.com/openai/codex.git"),
        Some(RepoRemote {
            provider: "github",
            owner: "openai".to_string(),
            repo: "codex".to_string(),
        })
    );
    assert_eq!(
        parse_repo_remote("https://dev.azure.com/acme/Project%20X/_git/codex.git"),
        Some(RepoRemote {
            provider: "azure_devops",
            owner: "acme/Project X".to_string(),
            repo: "codex".to_string(),
        })
    );
    assert_eq!(
        parse_repo_remote("git@ssh.dev.azure.com:v3/acme/Project%20X/codex"),
        Some(RepoRemote {
            provider: "azure_devops",
            owner: "acme/Project X".to_string(),
            repo: "codex".to_string(),
        })
    );
}

#[test]
fn azure_repos_by_repo_path_escapes_owner_slashes() {
    let remote = parse_repo_remote("https://dev.azure.com/acme/Project%20X/_git/codex.git")
        .expect("azure remote");

    assert_eq!(
        remote.by_repo_path(),
        "azure_devops/acme%2FProject%20X/codex"
    );
}
