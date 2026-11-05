"""Capacity observations must be private and unable to mask command failures."""
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import check as gate


class StorageObservationTests(unittest.TestCase):
    def test_real_capacity_contains_only_roles_and_numbers(self):
        with tempfile.TemporaryDirectory(prefix="private-storage-fixture-") as directory:
            output = io.StringIO()
            gate.report_storage("before-workspace-tests", Path(directory), output=output)
            text = output.getvalue()
            observation = json.loads(text)["storage_observation"]
            self.assertEqual(set(observation["locations"]), {"workspace", "temporary"})
            for fact in observation["locations"].values():
                self.assertTrue(fact["available"])
                for name in ("total_bytes", "used_bytes", "free_bytes"):
                    self.assertIs(type(fact[name]), int)
                    self.assertGreaterEqual(fact[name], 0)
            self.assertNotIn(directory, text)
            self.assertNotIn(str(gate.ROOT), text)

    def test_os_error_details_are_not_copied_to_the_observation(self):
        error = PermissionError(13, "private-path credential-must-not-escape")
        output = io.StringIO()
        with patch.object(gate.shutil, "disk_usage", side_effect=error):
            gate.report_storage("after-workspace-tests", Path("."), output=output)
        text = output.getvalue()
        for fact in json.loads(text)["storage_observation"]["locations"].values():
            self.assertEqual(fact, {"available": False, "errno": 13, "winerror": None})
        self.assertNotIn("private-path", text)
        self.assertNotIn("credential-must-not-escape", text)

    def test_broken_or_closed_output_keeps_the_original_command_error(self):
        class BrokenOutput:
            def write(self, _text):
                raise BrokenPipeError("private writer detail")

        closed = io.StringIO()
        closed.close()
        for output in (BrokenOutput(), closed):
            with self.subTest(output=type(output).__name__):
                original = RuntimeError("original command failed")
                with self.assertRaises(RuntimeError) as caught:
                    try:
                        raise original
                    finally:
                        gate.report_storage("after-workspace-tests", Path("."), output=output)
                self.assertIs(caught.exception, original)


if __name__ == "__main__":
    unittest.main()
