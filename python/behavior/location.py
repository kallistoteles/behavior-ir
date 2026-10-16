"""Source locations of the author's code (the first frame outside this package).

A location is either an absolute path to a real file or a marker for code without a file:
`<string>` (`exec`), `<stdin>`, or `<module name>` for a class whose module has no file (such as
`<__main__>` in a notebook or REPL). A marker is never resolved against the working directory (behavior-ir#19).

Known limitation, until behavior-ir-core#31: Core relativizes a module's locations (without an
explicit `root`) and an ad-hoc read's locations against the common directory of *all* their
files, markers included. When markers and real files are mixed, the real paths therefore keep
their checkout-dependent directories. Locations are outside semantics: no module hash changes,
but such a module's wire JSON, and the `record_id` of such an ad-hoc read, depend on the
checkout. Passing `root=` to `BehaviorModule` avoids it for modules.
"""

from __future__ import annotations

import os
import sys
from typing import Any

_PACKAGE_DIR = os.path.dirname(os.path.abspath(__file__))


def is_marker(filename: str) -> bool:
    """Whether `filename` names no file: `<string>`, `<stdin>`, `<cell>` and the like."""
    return filename.startswith("<") and filename.endswith(">")


def source_file(filename: str) -> str:
    """A code object's filename as a location: absolute for a file, verbatim for a marker."""
    return filename if is_marker(filename) else os.path.abspath(filename)


def code_loc(code: Any) -> tuple[str, int]:
    """The location of a function's code object."""
    return source_file(code.co_filename), code.co_firstlineno


def definition_loc(cls: type) -> tuple[str, int]:
    """Where class `cls` is defined: its module's file and `cls.__firstlineno__`, or the marker
    `<module name>` when the module has no file (a notebook cell, a REPL, an unregistered or
    `exec`'d module). Both parts come from the definition, never from where it is used."""
    module = sys.modules.get(cls.__module__)
    file = getattr(module, "__file__", None)
    line = getattr(cls, "__firstlineno__", 1)
    if isinstance(file, str):
        return source_file(file), line
    return f"<{cls.__module__}>", line


def caller_loc() -> tuple[str, int]:
    """The location of the nearest frame that is not inside the behavior package."""
    frame = sys._getframe(1)
    while frame is not None:
        filename = source_file(frame.f_code.co_filename)
        if os.path.dirname(filename) != _PACKAGE_DIR:
            return filename, frame.f_lineno
        frame = frame.f_back  # type: ignore[assignment]
    return "<unknown>", 0
