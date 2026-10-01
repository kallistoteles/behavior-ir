"""The `behavior` console script (feature 008): the engine's own command line, run in-process."""

from __future__ import annotations

import sys

from . import _engine


def main() -> None:
    sys.stdout.flush()
    sys.exit(_engine.cli(sys.argv[1:]))


if __name__ == "__main__":
    main()
