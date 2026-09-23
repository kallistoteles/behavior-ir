"""Exceptions raised by the authoring layer."""

from __future__ import annotations

from typing import Any


class BehaviorError(Exception):
    """Base class for all behavior errors."""

    def __init__(self, message: str, file: str | None = None, line: int | None = None) -> None:
        self.message = message
        self.file = file
        self.line = line
        where = f"{file}:{line}: " if file is not None and line is not None else ""
        super().__init__(f"{where}{message}")


class BehaviorTypeError(BehaviorError):
    """An expression is ill-typed; raised where the expression is built."""

    def __init__(
        self, message: str, code: str, file: str | None = None, line: int | None = None
    ) -> None:
        self.code = code
        super().__init__(f"{code}: {message}", file, line)


class BehaviorDefinitionError(BehaviorError):
    """The DSL was misused (control flow on symbolic values, bad field types, ...)."""


class BehaviorInvalid(BehaviorError):
    """The engine refused to admit the module."""

    def __init__(self, result: Any) -> None:
        self.result = result
        errors = "; ".join(f"{e.code}: {e.message}" for e in result.errors)
        super().__init__(f"behavior module was not admitted: {errors}")


class IntentRejected(BehaviorError):
    """A structured intent was rejected before evaluation."""

    def __init__(self, errors: list[dict[str, Any]]) -> None:
        self.errors = errors
        super().__init__("intent rejected: " + "; ".join(e["code"] for e in errors))
