use std::fs;

use pretty_assertions::assert_eq;
use tempfile::tempdir;

use super::resolve_repository_root;

#[test]
fn resolve_repository_root_does_not_spawn_git_show_toplevel() -> anyhow::Result<()> {
    let temp = tempdir()?;
    let repo = temp.path().join("repo");
    let nested = repo.join("nested");
    fs::create_dir_all(&nested)?;
    fs::create_dir(repo.join(".git"))?;

    let root = resolve_repository_root(&nested)?;

    assert_eq!(root, repo);
    Ok(())
}
