"""The engine's conformance suite against a plain Python backend (feature 005, SC-003)."""

from __future__ import annotations

import copy
from typing import Any

from behavior import InMemoryBackend, run_conformance

Doc = dict[str, Any]


class DictBackend:
    """The backend contract with dicts: immutable versions tagged by position, records by
    position, and one atomic compare-and-set commit."""

    def __init__(self) -> None:
        self._genesis: Doc | None = None
        self._head: Doc | None = None
        self._versions: dict[tuple[str, str], list[Doc]] = {}
        self._records: list[Doc] = []

    def genesis(self) -> Doc | None:
        return copy.deepcopy(self._genesis)

    def head(self) -> Doc | None:
        return copy.deepcopy(self._head)

    def create(self, genesis: Doc, head: Doc, seed: list[Doc]) -> None:
        assert self._genesis is None, "already created"
        self._genesis, self._head = genesis, head
        for v in seed:
            self._versions.setdefault((v["entity"], v["id"]), []).append(v)

    def version_at(self, key: Doc, position: int) -> Doc | None:
        vs = [v for v in self._versions.get((key["entity"], key["id"]), []) if v["created_at"] <= position]
        return copy.deepcopy(vs[-1]) if vs else None

    def version(self, key: Doc, revision: int) -> Doc | None:
        for v in self._versions.get((key["entity"], key["id"]), []):
            if v["revision"] == revision:
                return copy.deepcopy(v)
        return None

    def record(self, position: int) -> Doc | None:
        return copy.deepcopy(self._records[position - 1]) if 1 <= position <= len(self._records) else None

    def commit(self, expected: str, versions: list[Doc], record: Doc, head: Doc) -> str:
        assert self._head is not None
        if self._head["last_record"] != expected:
            return "head_moved"
        for v in versions:
            self._versions.setdefault((v["entity"], v["id"]), []).append(v)
        self._records.append(record)
        self._head = head
        return "applied"


class IgnoresHead(DictBackend):
    def commit(self, expected: str, versions: list[Doc], record: Doc, head: Doc) -> str:
        assert self._head is not None
        return super().commit(self._head["last_record"], versions, record, head)


class LatestReads(DictBackend):
    def version_at(self, key: Doc, position: int) -> Doc | None:
        return super().version_at(key, 1 << 62)


class ReordersRecords(DictBackend):
    def record(self, position: int) -> Doc | None:
        return super().record({1: 2, 2: 1}.get(position, position))


def test_reference_and_dict_backends_pass_every_case() -> None:
    for factory in (InMemoryBackend, DictBackend):
        report = run_conformance(factory)
        assert len(report.cases) == 17
        assert report.ok, report.failed()


def test_broken_python_backends_fail_their_case() -> None:
    for backend, case in [
        (IgnoresHead, "conflict_on_outdated_parent"),
        (LatestReads, "snapshot_consistency"),
        (ReordersRecords, "history_between_states"),
    ]:
        report = run_conformance(backend)
        result = {name: (ok, msg) for name, ok, msg in report.cases}[case]
        assert not result[0], f"{backend.__name__} must fail {case}"
        assert result[1]


class FailingHead(DictBackend):
    """Fails `head()` once created, like a storage outage."""

    def head(self) -> Doc | None:
        if self._genesis is not None and getattr(self, "down", False):
            raise OSError("storage unavailable")
        return super().head()


def test_backend_errors_surface_as_commit_refused() -> None:
    from behavior import CommitRefused, Store
    from examples.ledger.behavior import model

    backend = FailingHead()
    seed = [{"entity": "Account", "value": {"id": "a1", "active": True, "balance": "1.00"}}]
    store = Store.create(backend, model, Store.genesis_for(model, seed))
    backend.down = True
    try:
        store.current()
    except CommitRefused as e:
        assert e.code == "BACKEND_ERROR"
    else:
        raise AssertionError("expected CommitRefused")
