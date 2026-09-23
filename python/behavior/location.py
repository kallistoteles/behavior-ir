"""Source locations of the author's code (the first frame outside this package)."""

from __future__ import annotations

import os
import sys

_PACKAGE_DIR = os.path.dirname(os.path.abspath(__file__))


def caller_loc() -> tuple[str, int]:
    """Absolute file and line of the nearest frame that is not inside the behavior package."""
    frame = sys._getframe(1)
    while frame is not None:
        filename = os.path.abspath(frame.f_code.co_filename)
        if os.path.dirname(filename) != _PACKAGE_DIR:
            return filename, frame.f_lineno
        frame = frame.f_back  # type: ignore[assignment]
    return "<unknown>", 0
