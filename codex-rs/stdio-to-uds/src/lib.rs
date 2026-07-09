#![deny(clippy::print_stdout)]

use std::io;
use std::path::Path;

use anyhow::Context;
use codex_uds::UnixStream;
use tokio::io::AsyncWriteExt;

/// Connects to the Unix Domain Socket at `socket_path` and relays data between
/// standard input/output and the socket.
pub async fn run(socket_path: &Path) -> anyhow::Result<()> {
    let stream = UnixStream::connect(socket_path)
        .await
        .with_context(|| format!("failed to connect to socket at {}", socket_path.display()))?;
    let (mut socket_reader, mut socket_writer) = tokio::io::split(stream);

    let copy_socket_to_stdout = async {
        let mut stdout = tokio::io::stdout();
        tokio::io::copy(&mut socket_reader, &mut stdout).await?;
        stdout.flush().await?;
        anyhow::Ok(())
    };
    let copy_stdin_to_socket = async {
        let mut stdin = tokio::io::stdin();
        let bytes_copied = tokio::io::copy(&mut stdin, &mut socket_writer)
            .await
            .context("failed to copy data from stdin to socket")?;

        // The peer can close immediately after sending its response; in that
        // race, half-closing our write side can report NotConnected on some
        // platforms.
        if let Err(err) = socket_writer.shutdown().await
            && err.kind() != io::ErrorKind::NotConnected
        {
            return Err(err).context("failed to shutdown socket writer");
        }

        anyhow::Ok(bytes_copied)
    };

    tokio::pin!(copy_socket_to_stdout);
    tokio::pin!(copy_stdin_to_socket);

    let mut socket_to_stdout_done = false;
    let bytes_copied = tokio::select! {
        result = &mut copy_stdin_to_socket => result,
        result = &mut copy_socket_to_stdout => {
            result.context("failed to relay data from socket to stdout")?;
            socket_to_stdout_done = true;
            copy_stdin_to_socket.await
        }
    }
    .context("failed to relay data between stdin and socket")?;

    if bytes_copied == 0 {
        return Ok(());
    }

    if !socket_to_stdout_done {
        copy_socket_to_stdout
            .await
            .context("failed to relay data from socket to stdout")?;
    }

    Ok(())
}
