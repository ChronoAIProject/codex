#!/usr/bin/env python3

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / ".github" / "workflows" / "issue-deduplicator.yml"


class IssueDeduplicatorWorkflowTest(unittest.TestCase):
    def test_closed_issues_are_context_only(self) -> None:
        workflow = WORKFLOW.read_text(encoding="utf-8")

        self.assertIn(
            "- Treat closed issues as context only; do not return them as duplicates.",
            workflow,
        )
        self.assertNotIn(
            "- Closed issues can still be valid duplicates if they clearly match.",
            workflow,
        )

    def test_comment_does_not_ask_reporters_to_close_their_issue(self) -> None:
        workflow = WORKFLOW.read_text(encoding="utf-8")

        self.assertIn(
            "Potential duplicates detected. Please review whether these reports describe the same issue.",
            workflow,
        )
        self.assertNotIn(
            "Please review them and close your issue if it is a duplicate.",
            workflow,
        )


if __name__ == "__main__":
    unittest.main()
