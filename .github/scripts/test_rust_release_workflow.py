import re
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
RUST_RELEASE_WORKFLOW = REPO_ROOT / ".github" / "workflows" / "rust-release.yml"


class RustReleaseWorkflowTest(unittest.TestCase):
    def test_linux_sigstore_assets_do_not_share_codex_archive_prefix(self) -> None:
        workflow = RUST_RELEASE_WORKFLOW.read_text(encoding="utf-8")

        self.assertNotRegex(
            workflow,
            re.compile(r'"\$dest/\$\{binary\}-\$\{\{ matrix\.target \}\}\.sigstore"'),
        )
        self.assertIn(
            '"$dest/sigstore-${binary}-${{ matrix.target }}.sigstore"',
            workflow,
        )


if __name__ == "__main__":
    unittest.main()
