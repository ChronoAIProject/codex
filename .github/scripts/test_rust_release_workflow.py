#!/usr/bin/env python3

from pathlib import Path
import re
import unittest


REPO_ROOT = Path(__file__).resolve().parents[2]
RUST_RELEASE_WORKFLOW = REPO_ROOT / ".github/workflows/rust-release.yml"


def workflow_matrix_entries() -> list[tuple[int, dict[str, str]]]:
    entries: list[tuple[int, dict[str, str]]] = []
    current: dict[str, str] | None = None
    current_line = 0

    for line_number, raw_line in enumerate(
        RUST_RELEASE_WORKFLOW.read_text(encoding="utf-8").splitlines(), start=1
    ):
        line = raw_line.strip()
        if line.startswith("- "):
            if current is not None:
                entries.append((current_line, current))
            current = {}
            current_line = line_number
            parse_entry_line(line.removeprefix("- "), current)
        elif current is not None:
            parse_entry_line(line, current)

    if current is not None:
        entries.append((current_line, current))

    return entries


def parse_entry_line(line: str, entry: dict[str, str]) -> None:
    match = re.fullmatch(r"([A-Za-z_][A-Za-z0-9_]*):\s*(.*)", line)
    if match is None:
        return

    key, value = match.groups()
    if (value.startswith('"') and value.endswith('"')) or (
        value.startswith("'") and value.endswith("'")
    ):
        value = value[1:-1]
    entry[key] = value


class RustReleaseWorkflowTest(unittest.TestCase):
    def test_macos_primary_bundles_ship_code_mode_host(self) -> None:
        required_binaries = {
            "codex",
            "codex-code-mode-host",
            "codex-responses-api-proxy",
        }
        macos_primary_entries = [
            (line_number, entry)
            for line_number, entry in workflow_matrix_entries()
            if entry.get("bundle") == "primary"
            and entry.get("target", "").endswith("-apple-darwin")
            and "binaries" in entry
        ]

        self.assertGreaterEqual(len(macos_primary_entries), 1)
        missing = [
            f"line {line_number}: {entry['binaries']}"
            for line_number, entry in macos_primary_entries
            if not required_binaries.issubset(set(entry["binaries"].split()))
        ]
        self.assertEqual([], missing)


if __name__ == "__main__":
    unittest.main()
