#!/usr/bin/env python3

from pathlib import Path
import re
import unittest


REPO_ROOT = Path(__file__).resolve().parents[2]
RUST_RELEASE_WORKFLOW = REPO_ROOT / ".github" / "workflows" / "rust-release.yml"


class RustReleaseWorkflowTest(unittest.TestCase):
    def test_signed_macos_artifact_verification_assesses_gatekeeper_acceptance(
        self,
    ) -> None:
        workflow = RUST_RELEASE_WORKFLOW.read_text(encoding="utf-8")
        match = re.search(
            r"verify_signed_binary\(\) \{\n(?P<body>.*?)\n          \}",
            workflow,
            flags=re.DOTALL,
        )

        self.assertIsNotNone(match)
        body = match.group("body")
        self.assertIn("codesign --verify --strict --verbose=2 \"$path\"", body)
        self.assertIn("spctl --assess --type execute --verbose=4 \"$path\"", body)


if __name__ == "__main__":
    unittest.main()
