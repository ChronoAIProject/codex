import plistlib
import unittest
from pathlib import Path


ENTITLEMENTS_PATH = (
    Path(__file__).parent / "macos-signing" / "codex.entitlements.plist"
)


class MacosSigningEntitlementsTest(unittest.TestCase):
    def test_codex_entitlements_support_v8_executable_memory(self) -> None:
        with ENTITLEMENTS_PATH.open("rb") as entitlements_file:
            entitlements = plistlib.load(entitlements_file)

        self.assertEqual(
            {
                "com.apple.security.cs.allow-jit": True,
                "com.apple.security.cs.allow-unsigned-executable-memory": True,
            },
            {
                entitlement: entitlements.get(entitlement)
                for entitlement in (
                    "com.apple.security.cs.allow-jit",
                    "com.apple.security.cs.allow-unsigned-executable-memory",
                )
            },
        )


if __name__ == "__main__":
    unittest.main()
