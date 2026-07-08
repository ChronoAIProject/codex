import re
import unittest
from pathlib import Path


INSTALL_PS1 = Path(__file__).with_name("install.ps1")


class InstallPs1ChecksumTest(unittest.TestCase):
    def test_archive_digest_uses_dotnet_sha256(self) -> None:
        script = INSTALL_PS1.read_text(encoding="utf-8")
        match = re.search(
            r"function Test-ArchiveDigest \{(?P<body>.*?)\n\}",
            script,
            flags=re.DOTALL,
        )
        self.assertIsNotNone(match)

        body = match.group("body")
        self.assertNotIn("Get-FileHash", body)
        self.assertIn("[System.Security.Cryptography.SHA256]::Create()", body)
        self.assertIn("[System.IO.File]::OpenRead($ArchivePath)", body)


if __name__ == "__main__":
    unittest.main()
