#!/bin/sh

set -eu

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"

CODEX_INSTALL_TEST_SOURCE_ONLY=1 . "$script_dir/install.sh"

download_text() {
  cat <<'EOF'
{"assets":[{"name":"codex-package_SHA256SUMS","digest":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},{"name":"codex-package-x86_64-unknown-linux-musl.tar.gz","digest":"sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"},{"name":"codex-package-aarch64-unknown-linux-musl.tar.gz","digest":"sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"}]}
EOF
}

actual="$(release_asset_digest_or_empty "codex-package-x86_64-unknown-linux-musl.tar.gz" "0.143.0")"
expected="bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"

if [ "$actual" != "$expected" ]; then
  echo "expected $expected, got $actual" >&2
  exit 1
fi
