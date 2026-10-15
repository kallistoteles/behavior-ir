"""The engine's conformance suite against a plain Python backend (feature 005, SC-003)."""

from __future__ import annotations

import copy
from typing import Any

from behavior import InMemoryBackend, run_conformance

Doc = dict[str, Any]

# The cases of Core 0.13.0's conformance suite (019 added the `creation_*` cases).
CASES = [
    "create_and_open", "commit_new_state", "conflict_on_outdated_parent",
    "no_partial_application", "crash_retry", "snapshot_consistency", "store_binding",
    "derived_dependencies", "load_at_past_state", "history_between_states",
    "idempotent_resubmission", "no_op_transition", "identity_is_content", "evidence_policy",
    "evidence_atomicity", "replay_detects_tamper", "observed_read_set", "create_and_remove_entity",
    "identity_never_reused", "removed_entity_history", "referential_integrity_on_resulting_state",
    "reference_index_consistency", "concurrent_creation_same_identity", "existence_snapshot",
    "query_snapshot", "query_index_consistency", "query_order_independence",
    "module_invariant_preserved", "migration_atomicity", "schema_history_consistency",
    "command_history", "creation_complete_product", "creation_invalid_seed_no_publish",
    "creation_incomplete_product", "creation_before_publication_failure",
    "creation_lost_ack_recovery", "creation_equivalent_retry", "creation_conflicting_retry",
    "creation_advanced_recovery", "creation_migrated_recovery", "creation_competing_equivalent",
    "creation_competing_conflicting", "creation_order_independence",
    "creation_publication_read_interleaving", "creation_recovery_read_failure",
]


class DictBackend:
    """The backend contract with dicts: immutable versions tagged by position, records by
    position, removal positions and reference-index edge events (feature 006), and one atomic
    compare-and-set commit."""

    def __init__(self) -> None:
        self._genesis: Doc | None = None
        self._head: Doc | None = None
        self._created: Doc | None = None  # the head of the original creation (position 0)
        self._versions: dict[tuple[str, str], list[Doc]] = {}
        self._records: list[Doc] = []
        self._removed: dict[tuple[str, str], int] = {}
        # Edge events: [target, source, field, added_at, dropped_at | None].
        self._edges: list[list[Any]] = []

    def _apply_refs(self, changes: list[Doc], position: int) -> None:
        for c in changes:
            target = (c["target"]["entity"], c["target"]["id"])
            source = (c["source"]["entity"], c["source"]["id"])
            if c["op"] == "add":
                self._edges.append([target, source, c["field"], position, None])
            else:
                for e in self._edges:
                    if e[0] == target and e[1] == source and e[2] == c["field"] and e[4] is None:
                        e[4] = position
                        break

    def genesis(self) -> Doc | None:
        return copy.deepcopy(self._genesis)

    def head(self) -> Doc | None:
        return copy.deepcopy(self._head)

    def create(self, genesis: Doc, head: Doc, seed: list[Doc], seed_refs: list[Doc]) -> None:
        """Publishes the original creation, or recovers it (Core 019): a retry of the identical
        creation writes nothing and keeps all later history; any other creation is refused."""
        candidate = DictBackend()  # built privately: nothing is visible until it is complete
        candidate._genesis, candidate._head, candidate._created = genesis, head, head
        for v in seed:
            candidate._versions.setdefault((v["entity"], v["id"]), []).append(v)
        candidate._apply_refs(seed_refs, 0)
        if self._genesis is None:
            self.__dict__.update(copy.deepcopy(candidate.__dict__))
            return
        if self._genesis != genesis:
            raise ValueError("conflicting Store creation identity")
        for entity in genesis["entity_declarations"]:  # key order is never semantic
            if (sorted(k["id"] for k in self.keys_at(entity, 0))
                    != sorted(k["id"] for k in candidate.keys_at(entity, 0))):
                raise ValueError("original creation universe differs")
        for v in seed:
            key = {"entity": v["entity"], "id": v["id"]}
            if (self.version_at(key, 0) != v or self.version(key, 1) != v
                    or self.incoming_at(key, 0) != candidate.incoming_at(key, 0)):
                raise ValueError("original creation product differs")
        if (self._created or {}).get("state_ref") != head["state_ref"]:
            raise ValueError("original creation state anchor differs")

    def removed_at(self, key: Doc) -> int | None:
        return self._removed.get((key["entity"], key["id"]))

    def incoming_at(self, target: Doc, position: int) -> list[Doc]:
        t = (target["entity"], target["id"])
        out = [
            {"entity": e[1][0], "id": e[1][1], "field": e[2]}
            for e in self._edges
            if e[0] == t and e[3] <= position and (e[4] is None or e[4] > position)
        ]
        return sorted(out, key=lambda x: (x["entity"], x["id"], x["field"]))

    def version_at(self, key: Doc, position: int) -> Doc | None:
        vs = [v for v in self._versions.get((key["entity"], key["id"]), []) if v["created_at"] <= position]
        return copy.deepcopy(vs[-1]) if vs else None

    def keys_at(self, entity_type: str, position: int) -> list[Doc]:
        """Feature 007: the type index, every entity of the type existing at `position`."""
        out = []
        for (entity, id_), vs in self._versions.items():
            removed = self._removed.get((entity, id_))
            if (entity == entity_type and vs[0]["created_at"] <= position
                    and (removed is None or removed > position)):
                out.append({"entity": entity, "id": id_})
        return sorted(out, key=lambda k: k["id"])

    def version(self, key: Doc, revision: int) -> Doc | None:
        for v in self._versions.get((key["entity"], key["id"]), []):
            if v["revision"] == revision:
                return copy.deepcopy(v)
        return None

    def record(self, position: int) -> Doc | None:
        return copy.deepcopy(self._records[position - 1]) if 1 <= position <= len(self._records) else None

    def commit(
        self, expected: str, versions: list[Doc], removals: list[Doc], ref_changes: list[Doc],
        record: Doc, head: Doc,
    ) -> str:
        assert self._head is not None
        if self._head["last_record"] != expected:
            return "head_moved"
        position = head["state_ref"]["position"]
        for v in versions:
            self._versions.setdefault((v["entity"], v["id"]), []).append(v)
        for k in removals:
            self._removed.setdefault((k["entity"], k["id"]), position)
        self._apply_refs(ref_changes, position)
        self._records.append(record)
        self._head = head
        return "applied"


class IgnoresHead(DictBackend):
    def commit(
        self, expected: str, versions: list[Doc], removals: list[Doc], ref_changes: list[Doc],
        record: Doc, head: Doc,
    ) -> str:
        assert self._head is not None
        return super().commit(self._head["last_record"], versions, removals, ref_changes, record, head)


class LatestReads(DictBackend):
    def version_at(self, key: Doc, position: int) -> Doc | None:
        return super().version_at(key, 1 << 62)


class ReordersRecords(DictBackend):
    def record(self, position: int) -> Doc | None:
        return super().record({1: 2, 2: 1}.get(position, position))


class ForgetsRemovedIdentities(DictBackend):
    """Feature 006: `used_at` forgets an identity once it is removed."""

    def used_at(self, key: Doc, position: int) -> bool:
        if self.removed_at(key) is not None:
            return False
        return self.version_at(key, position) is not None


class CurrentOnlyIndex(DictBackend):
    """Feature 006: `incoming_at` ignores the position."""

    def incoming_at(self, target: Doc, position: int) -> list[Doc]:
        return super().incoming_at(target, 1 << 62)


class StaleIndex(DictBackend):
    """Feature 006: dropped references are never dropped from the index."""

    def commit(
        self, expected: str, versions: list[Doc], removals: list[Doc], ref_changes: list[Doc],
        record: Doc, head: Doc,
    ) -> str:
        adds = [c for c in ref_changes if c["op"] == "add"]
        return super().commit(expected, versions, removals, adds, record, head)


def test_reference_and_dict_backends_pass_every_case() -> None:
    for factory in (InMemoryBackend, DictBackend):
        report = run_conformance(factory)
        assert [name for name, _, _ in report.cases] == CASES
        assert report.ok, report.failed()


def test_current_core_collects_command_history_conformance_for_both_backends() -> None:
    """Feature 500: durable command history runs through the shared suite and Python backend."""
    for factory in (InMemoryBackend, DictBackend):
        report = run_conformance(factory)
        cases = {name: (ok, message) for name, ok, message in report.cases}
        assert list(cases) == CASES
        assert "command_history" in cases
        ok, message = cases["command_history"]
        assert ok, message
        assert report.ok, report.failed()


def test_broken_python_backends_fail_their_case() -> None:
    for backend, case in [
        (IgnoresHead, "conflict_on_outdated_parent"),
        (LatestReads, "snapshot_consistency"),
        (ReordersRecords, "history_between_states"),
        (ForgetsRemovedIdentities, "identity_never_reused"),
        (CurrentOnlyIndex, "existence_snapshot"),
        (StaleIndex, "reference_index_consistency"),
    ]:
        report = run_conformance(backend)
        result = {name: (ok, msg) for name, ok, msg in report.cases}[case]
        assert not result[0], f"{backend.__name__} must fail {case}"
        assert result[1]


class IndexedBackend(DictBackend):
    """Feature 007: with the optional field index (here a scan of the type index)."""

    def keys_by_field_at(self, entity_type: str, field: str, value: Any, position: int) -> list[Doc]:
        return [
            k for k in self.keys_at(entity_type, position)
            if (v := self.version_at(k, position)) is not None and v["value"].get(field) == value
        ]


class CurrentOnlyKeys(DictBackend):
    """Feature 007: `keys_at` ignores the position."""

    def keys_at(self, entity_type: str, position: int) -> list[Doc]:
        return super().keys_at(entity_type, 1 << 62)


class StaleFieldIndex(IndexedBackend):
    """Feature 007: the field index is never updated after genesis."""

    def keys_by_field_at(self, entity_type: str, field: str, value: Any, position: int) -> list[Doc]:
        return super().keys_by_field_at(entity_type, field, value, 0)


class ReversedKeys(IndexedBackend):
    """Feature 007, a positive control: index order is never semantic."""

    def keys_at(self, entity_type: str, position: int) -> list[Doc]:
        return list(reversed(super().keys_at(entity_type, position)))

    def keys_by_field_at(self, entity_type: str, field: str, value: Any, position: int) -> list[Doc]:
        return list(reversed(super().keys_by_field_at(entity_type, field, value, position)))


def test_query_backends() -> None:
    for backend, case in [
        (CurrentOnlyKeys, "query_snapshot"),
        (StaleFieldIndex, "query_index_consistency"),
    ]:
        report = run_conformance(backend)
        result = {name: (ok, msg) for name, ok, msg in report.cases}[case]
        assert not result[0], f"{backend.__name__} must fail {case}"
        assert result[1]
    for backend in (IndexedBackend, ReversedKeys):
        report = run_conformance(backend)
        assert report.ok, report.failed()


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
