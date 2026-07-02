use std::path::PathBuf;
use std::process::Command;

#[test]
fn musl_build_does_not_pull_in_openssl() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = Command::new(cargo)
        .current_dir(manifest_dir)
        .args([
            "tree",
            "-p",
            "codex-exec-server",
            "--target",
            "x86_64-unknown-linux-musl",
            "-i",
            "openssl-sys",
        ])
        .output()
        .expect("run cargo tree for codex-exec-server musl dependency graph");

    assert!(
        !output.status.success(),
        "openssl-sys should not be in the codex-exec-server musl dependency graph:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("package ID specification `openssl-sys` did not match any packages"),
        "unexpected cargo tree failure:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
