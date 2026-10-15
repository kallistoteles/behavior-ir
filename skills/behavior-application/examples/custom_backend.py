"""A custom storage backend. A host may keep Behavior's documents anywhere, but a backend is
trusted only after it passes the engine's conformance suite. Only `create` and `commit` write;
`commit` must be one atomic compare-and-set, and `create` must accept a retry of the identical
original creation without writing (Core 019)."""

import copy
from typing import Any

from behavior import run_conformance

Doc = dict[str, Any]


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


report = run_conformance(DictBackend)
assert report.ok, report.failed()
# Core 019: creation must be recoverable; an identical retry of `create` writes nothing.
assert "creation_lost_ack_recovery" in [name for name, _, _ in report.cases]
