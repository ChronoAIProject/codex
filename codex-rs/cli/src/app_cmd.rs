use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Parser)]
pub struct AppCommand {
    /// Workspace path to open in Codex Desktop.
    #[arg(value_name = "PATH", default_value = ".")]
    pub path: PathBuf,

    /// Override the app installer download URL (advanced).
    #[arg(long = "download-url")]
    pub download_url_override: Option<String>,
}

pub async fn run_app(cmd: AppCommand) -> anyhow::Result<()> {
    let workspace = std::fs::canonicalize(&cmd.path).unwrap_or(cmd.path);
    #[cfg(target_os = "macos")]
    {
        crate::desktop_app::run_app_open_or_install(workspace, cmd.download_url_override).await
    }
    #[cfg(target_os = "windows")]
    {
        crate::desktop_app::run_app_open_or_install(workspace, cmd.download_url_override).await
    }
}

#[cfg(test)]
mod tests {
    use super::AppCommand;
    use clap::Parser as _;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;

    #[test]
    fn app_command_does_not_default_download_url() {
        let cmd = AppCommand::try_parse_from(["codex"]).expect("app command should parse");

        assert_eq!(
            (cmd.path, cmd.download_url_override),
            (PathBuf::from("."), None)
        );
    }
}
