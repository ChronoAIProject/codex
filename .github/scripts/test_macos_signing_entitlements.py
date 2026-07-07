#!/usr/bin/env python3

import plistlib
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
CODEX_ENTITLEMENTS = ROOT / ".github/scripts/macos-signing/codex.entitlements.plist"


class MacosSigningEntitlementsTest(unittest.TestCase):
    def test_codex_entitlements_support_v8_hardened_runtime_jit(self) -> None:
        entitlements = plistlib.loads(CODEX_ENTITLEMENTS.read_bytes())

        self.assertEqual(
            {
                "com.apple.security.cs.allow-jit": True,
                "com.apple.security.cs.allow-unsigned-executable-memory": True,
            },
            entitlements,
        )


if __name__ == "__main__":
    unittest.main()
