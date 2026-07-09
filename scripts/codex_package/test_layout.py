#!/usr/bin/env python3

from pathlib import Path
import stat
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from codex_package.layout import build_package_dir
from codex_package.layout import validate_package_dir
from codex_package.targets import PACKAGE_VARIANTS
from codex_package.targets import PackageInputs
from codex_package.targets import TARGET_SPECS


class PackageLayoutTest(unittest.TestCase):
    def test_windows_codex_package_includes_wsl_entrypoint_resource(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            package_dir = root / "package"
            package_dir.mkdir()
            inputs = PackageInputs(
                entrypoint_bin=touch_executable(root / "codex.exe"),
                rg_bin=touch_executable(root / "rg.exe"),
                zsh_bin=None,
                bwrap_bin=None,
                codex_command_runner_bin=touch_executable(
                    root / "codex-command-runner.exe"
                ),
                codex_windows_sandbox_setup_bin=touch_executable(
                    root / "codex-windows-sandbox-setup.exe"
                ),
            )
            touch_executable(root / "codex")

            build_package_dir(
                package_dir,
                "1.2.3",
                PACKAGE_VARIANTS["codex"],
                TARGET_SPECS["x86_64-pc-windows-msvc"],
                inputs,
            )
            validate_package_dir(
                package_dir,
                PACKAGE_VARIANTS["codex"],
                TARGET_SPECS["x86_64-pc-windows-msvc"],
                include_zsh=False,
            )

            self.assertTrue((package_dir / "bin" / "codex.exe").is_file())
            self.assertTrue((package_dir / "codex-resources" / "codex").is_file())


def touch_executable(path: Path) -> Path:
    path.touch()
    path.chmod(path.stat().st_mode | stat.S_IXUSR)
    return path


if __name__ == "__main__":
    unittest.main()
