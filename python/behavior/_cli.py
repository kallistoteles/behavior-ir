"""The `behavior` console script: the core's own command-line tool, bundled with the package as
a binary (feature 011, FR-006c). The binding does not link the command line; this launcher only
replaces itself with the bundled program."""

from __future__ import annotations

import os
import sys
from importlib.resources import files
from pathlib import Path


def binary() -> Path:
    """The bundled `behavior` program. `BEHAVIOR_CLI_BINARY` points elsewhere (development and
    tests only)."""
    override = os.environ.get("BEHAVIOR_CLI_BINARY")
    if override:
        return Path(override)
    return Path(str(files("behavior") / "_bin" / "behavior"))


def main() -> None:
    path = binary()
    if not path.is_file():
        print(f"behavior: the bundled core CLI is missing ({path})", file=sys.stderr)
        sys.exit(2)
    sys.stdout.flush()
    os.execv(path, [str(path), *sys.argv[1:]])


if __name__ == "__main__":
    main()
