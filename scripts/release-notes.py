#!/usr/bin/env python3
"""Extract one exact tagged version from Keep a Changelog, or fail closed."""

import pathlib
import re
import sys


def release_notes(changelog, tag):
    version = tag.removeprefix("v")
    if not re.fullmatch(r"\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?", version):
        raise ValueError(f"invalid release tag: {tag}")
    sections = re.split(r"(?m)^## ", changelog)
    matches = [s for s in sections[1:] if s.startswith(f"[{version}] - ")]
    if len(matches) != 1:
        raise ValueError(f"expected exactly one CHANGELOG section for {tag}")
    body = matches[0].split("\n", 1)[1]
    body = re.split(r"(?m)^\[[^\]]+\]:", body)[0].strip()
    if not body:
        raise ValueError(f"empty CHANGELOG section for {tag}")
    return body + "\n"


if __name__ == "__main__":
    try:
        print(release_notes(pathlib.Path("CHANGELOG.md").read_text(), sys.argv[1]), end="")
    except (ValueError, IndexError) as error:
        sys.exit(str(error))
