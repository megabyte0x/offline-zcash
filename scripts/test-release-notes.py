import importlib.util
import pathlib
import unittest

spec = importlib.util.spec_from_file_location("release_notes", pathlib.Path(__file__).with_name("release-notes.py"))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class ReleaseNotesTest(unittest.TestCase):
    def test_exact_section_without_next_release_or_links(self):
        changelog = "## [Unreleased]\n\n### Added\n- Later\n\n## [0.5.1] - 2026-10-27\n\n### Fixed\n- This release\n\n## [0.5.0] - 2026-10-27\n- Older\n\n[0.5.1]: https://example.com\n"
        self.assertEqual(module.release_notes(changelog, "v0.5.1"), "### Fixed\n- This release\n")
        self.assertEqual(module.release_notes(changelog, "v0.5.0"), "- Older\n")
        for bad in ("v0.5", "v0.5.10", "v0.6.0", "Unreleased"):
            with self.assertRaises(ValueError):
                module.release_notes(changelog, bad)

    def test_missing_empty_and_duplicate_sections_fail(self):
        for changelog in ("", "## [0.5.1] - 2026-10-27\n\n", "## [0.5.1] - date\n- A\n## [0.5.1] - date\n- B\n"):
            with self.assertRaises(ValueError):
                module.release_notes(changelog, "v0.5.1")


if __name__ == "__main__":
    unittest.main()
