use super::*;
use clap::Parser;
use clap::error::ErrorKind;
use pretty_assertions::assert_eq;

#[test]
fn parses_worktree_without_name() {
    let cli = Cli::parse_from(["codex-tui", "--worktree"]);

    assert_eq!(cli.worktree, Some(None));
    assert!(!cli.tmux);
}

#[test]
fn parses_tmux_worktree_name_and_prompt() {
    let cli = Cli::parse_from([
        "codex-tui",
        "--tmux",
        "--worktree",
        "fix-login",
        "investigate failing CI",
    ]);

    assert_eq!(
        (cli.tmux, cli.worktree, cli.prompt.as_deref()),
        (
            true,
            Some(Some("fix-login".to_string())),
            Some("investigate failing CI")
        )
    );
}

#[test]
fn tmux_requires_worktree() {
    let err = Cli::try_parse_from(["codex-tui", "--tmux"]).expect_err("expected parse error");

    assert_eq!(err.kind(), ErrorKind::MissingRequiredArgument);
}
