#!/usr/bin/env python3

import importlib.util
from pathlib import Path
import sys
import tempfile
import unittest


SCRIPT_PATH = Path(__file__).with_name("install-skill-from-github.py")
sys.path.insert(0, str(SCRIPT_PATH.parent))
SPEC = importlib.util.spec_from_file_location("install_skill_from_github", SCRIPT_PATH)
assert SPEC is not None
INSTALLER = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
sys.modules[SPEC.name] = INSTALLER
SPEC.loader.exec_module(INSTALLER)


class SkillSourcePathTest(unittest.TestCase):
    def test_resolves_regular_repo_skill_path(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            skill_dir = repo_root / "skills" / ".curated" / "demo"
            skill_dir.mkdir(parents=True)

            actual = INSTALLER._skill_source_path(
                str(repo_root), "skills/.curated/demo"
            )

            self.assertEqual(str(skill_dir), actual)

    def test_resolves_bundled_archive_skill_path(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            skill_dir = repo_root / "skills" / "skills" / ".curated" / "hatch-pet"
            skill_dir.mkdir(parents=True)

            actual = INSTALLER._skill_source_path(
                str(repo_root), "skills/.curated/hatch-pet"
            )

            self.assertEqual(str(skill_dir), actual)


if __name__ == "__main__":
    unittest.main()
