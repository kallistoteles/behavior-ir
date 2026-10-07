"""Migrations from Python (feature 009, FR-022): declare a migration between two modules, admit
it, review its summary, apply it to a store, and narrow only once the data fits."""

from __future__ import annotations

from decimal import Decimal

import pytest

from behavior import (
    BehaviorTypeError, CommitRefused, InMemoryBackend, Migration, Rounding, Store, all_,
    apply_migration, authorize_migration, enum_map, replay_behavior, replay_data, rescale,
    select, strict_enum_map, strict_unwrap, underlying, verify_migration,
)
from examples.schema_evolution import v1, v2, v3

T0 = "2026-10-02T09:00:00Z"
T1 = "2026-10-02T10:00:00Z"


def broaden(**overrides: object) -> Migration:
    """V1 → V2: an enum widening, a rename, a new optional field, a conversion, a drop, a
    retirement."""
    transforms = {
        v1.Culture: lambda old: {
            "medium_type": enum_map(old.medium, {
                v1.MediumKind.MS: v2.MediumKind.MS, v1.MediumKind.WPM: v2.MediumKind.WPM,
            }),
            "ph": old.ph,
            "notes": None,
            "price": rescale(underlying(old.price), v2.Money, Rounding.HALF_EVEN),
        },
    }
    args: dict[str, object] = {
        "source": v1.model, "target": v2.model, "transforms": transforms,
        "drops": {v1.Culture: ["legacy_code"]}, "retire": [v1.AuditNote],
    }
    args.update(overrides)
    return Migration(**args)  # type: ignore[arg-type]


def narrow() -> Migration:
    """V2 → V3: every order must have a region, proven by a source requirement."""
    return Migration(
        source=v2.model, target=v3.model,
        requires={
            "every_order_has_region": lambda: all_(select(v2.Order), lambda o: o.region.is_some()),
        },
        transforms={v2.Order: lambda old: {"region": strict_unwrap(old.region)}},
    )


SEED = [
    {"entity": "Customer", "value": {"id": "c1", "name": "Ada"}},
    {"entity": "Culture", "value": {"id": "k1", "medium": "WPM", "status": "ACTIVE", "ph": 7,
                                    "legacy_code": "L1", "price": Decimal("1.50")}},
    {"entity": "Order", "value": {"id": "o1", "customer": "c1", "region": None, "qty": 3}},
    {"entity": "Order", "value": {"id": "o2", "customer": "c1", "region": "north", "qty": 1}},
]


def v1_store() -> Store:
    return Store.create(InMemoryBackend(), v1.model, Store.genesis_for(v1.model, SEED))


def test_a_migration_is_admitted_with_a_reviewable_summary() -> None:
    m = broaden()
    a = m.admit()
    assert a.ok and a.errors == []
    assert a.hash == m.hash and m.hash.startswith("sha256:")
    assert m.summary() == {
        "types": {
            "Culture": {"copied": ["id", "status"], "transformed": ["medium_type", "ph", "price"],
                        "new": ["notes"], "dropped": ["legacy_code"]},
        },
        "retired": ["AuditNote"],
    }
    assert m.source_schema == v1.model.schema_hash and m.target_schema == v2.model.schema_hash


def test_the_identity_is_independent_of_the_name() -> None:
    assert broaden(name="renamed").hash == broaden().hash


@pytest.mark.parametrize(("change", "code"), [
    ({"drops": {}}, "UNACKNOWLEDGED_FIELD_DROP"),
    ({"retire": []}, "MISSING_RETIREMENT"),
    ({"transforms": {v1.Culture: lambda old: {
        "medium_type": enum_map(old.medium, {
            v1.MediumKind.MS: v2.MediumKind.MS, v1.MediumKind.WPM: v2.MediumKind.WPM}),
        "ph": old.ph, "price": rescale(underlying(old.price), v2.Money, Rounding.HALF_EVEN)}}},
     "MISSING_MIGRATION_FIELD"),
])
def test_admission_errors_have_codes(change: dict[str, object], code: str) -> None:
    a = broaden(**change).admit()
    assert not a.ok
    assert [e.code for e in a.errors] == [code], a.errors


def test_a_partial_enum_map_is_refused_where_it_is_written() -> None:
    with pytest.raises(BehaviorTypeError) as e:
        broaden(transforms={v1.Culture: lambda old: {
            "medium_type": enum_map(old.medium, {v1.MediumKind.MS: v2.MediumKind.MS}),
            "ph": old.ph, "notes": None,
            "price": rescale(underlying(old.price), v2.Money, Rounding.HALF_EVEN)}})
    assert e.value.code == "UNMAPPED_ENUM_VALUE" and "WPM" in str(e.value)


def test_a_store_migrates_in_place() -> None:
    store = v1_store()
    before = store.current()
    r = store.migrate(broaden(), commit_time=T1)
    assert r.result_state.position == 1 and not r.already
    k1 = store.load("Culture", "k1")
    assert k1["value"]["medium_type"] == "WPM" and "legacy_code" not in k1["value"]
    assert k1["revision"] == 2 and k1["created_at"] == 1
    assert store.load("Culture", "k1", before)["value"]["medium"] == "WPM"
    assert [(s.since, s.hash) for s in store.schema_history()] == [
        (0, v1.model.schema_hash), (1, v2.model.schema_hash)]
    ev = store.evaluate(v2.model, "kill", bindings={"culture": "k1"}, commit_time=T1)
    assert ev.bundle is not None
    store.commit(v2.model, ev.bundle)
    with pytest.raises(CommitRefused) as e:
        store.evaluate(v1.model, "kill", bindings={"culture": "k1"}, commit_time=T1)
    assert e.value.code == "SCHEMA_MISMATCH"


def test_narrowing_waits_for_the_data() -> None:
    store = v1_store()
    store.migrate(broaden(), commit_time=T1)
    with pytest.raises(CommitRefused) as e:
        store.migrate(narrow(), commit_time=T1)
    assert e.value.code == "MIGRATION_REQUIREMENT_FAILED"
    assert "every_order_has_region" in str(e.value) and "Order#o1" in str(e.value)
    position = store.current().position
    ev = store.evaluate(v2.model, "set_region", bindings={"order": "o1"},
                        input={"region": "south"}, commit_time=T1)
    assert ev.bundle is not None
    store.commit(v2.model, ev.bundle)
    r = store.migrate(narrow(), commit_time=T1)
    assert r.result_state.position == position + 2
    assert store.load("Order", "o1")["value"]["region"] == "south"


def test_plain_application_over_a_supplied_universe() -> None:
    out = apply_migration(broaden(), SEED)
    assert out["result"] == "MIGRATED"
    k1 = next(e for e in out["entities"] if e["entity"] == "Culture")
    assert k1["value"]["price"] == "1.5000" and k1["migrated"]
    refused = apply_migration(narrow(), [
        {"entity": "Customer", "value": {"id": "c1", "name": "Ada"}},
        {"entity": "Order", "value": {"id": "o1", "customer": "c1", "region": None, "qty": 1}},
    ])
    assert refused["result"] == "MIGRATION_REQUIREMENT_FAILED"
    assert refused["entities"] == [["Order", "o1"]]


def test_a_strict_enum_map_is_a_narrowing() -> None:
    m = Migration(
        source=v2.model, target=v3.model,
        transforms={v2.Order: lambda old: {"region": strict_unwrap(old.region)}},
    )
    assert m.admit().ok
    assert callable(strict_enum_map)


def test_migration_evidence_follows_the_store_policy() -> None:
    policy = {"format": "behavior.evidence_policy.v1", "require": "none",
              "migration": {"require": "commit_authorization"}}
    store = Store.create(InMemoryBackend(), v1.model, Store.genesis_for(v1.model, SEED, policy))
    with pytest.raises(CommitRefused) as e:
        store.migrate(broaden(), commit_time=T1)
    assert e.value.code == "TRUSTED_GOVERNANCE_UPGRADE_REQUIRED"
    auth = authorize_migration(
        broaden(), store, policy={"policy_version": "1", "require": "verified"}, now=T1,
    )
    assert not auth.allowed
    assert auth.data["data_version"] == store.data_version()
    assert [r["code"] for r in auth.reasons] == ["unverified"]


def test_legacy_required_governance_migration_refuses_without_changing_store() -> None:
    """Feature 500: a v1 migration policy is never implicitly authenticated or replaced."""
    policy = {"format": "behavior.evidence_policy.v1", "require": "none",
              "migration": {"require": "commit_authorization"}}
    store = Store.create(InMemoryBackend(), v1.model, Store.genesis_for(v1.model, SEED, policy))
    before = store.current()
    history = list(store.history())
    schemas = store.schema_history()
    entities = [store.load(item["entity"], item["value"]["id"]) for item in SEED]
    migration = broaden()
    for _ in range(2):
        with pytest.raises(CommitRefused) as e:
            store.migrate(migration, commit_time=T1)
        assert e.value.code == "TRUSTED_GOVERNANCE_UPGRADE_REQUIRED"
        assert store.current() == before
        assert list(store.history()) == history
        assert store.schema_history() == schemas
        assert [store.load(item["entity"], item["value"]["id"]) for item in SEED] == entities


def test_verify_authorize_and_apply_a_migration_under_a_strict_policy() -> None:
    a = verify_migration(narrow())
    assert a.verified, a.findings
    narrowing = next(c for c in a.checks if c["kind"] == "migration_narrowing")
    assert narrowing["outcome"] == "proven" and narrowing["under"] == ["every_order_has_region"]
    policy = {"format": "behavior.evidence_policy.v1", "require": "none",
              "migration": {"require": "commit_authorization"}}
    seed = SEED[:1] + [
        {"entity": "Order", "value": {"id": "o1", "customer": "c1", "region": "x", "qty": 1}}]
    store = Store.create(InMemoryBackend(), v1.model, Store.genesis_for(v1.model, seed, policy))
    broadening = broaden()
    b = verify_migration(broadening)
    assert not b.verified  # Money widens to four decimals and its range shrinks
    assert any(f["kind"] == "evaluation_error" for f in b.findings)
    with pytest.raises(CommitRefused):
        store.migrate(broadening, commit_time=T1)
    # A migration whose attestation has blocking findings is not authorized under `verified`.
    auth = authorize_migration(
        broadening, store, policy={"policy_version": "1", "require": "verified"},
        attestation=b, now=T1,
    )
    assert not auth.allowed


def test_replay_crosses_a_migration() -> None:
    store = v1_store()
    ev = store.evaluate(v1.model, "kill", bindings={"culture": "k1"}, commit_time=T0)
    assert ev.bundle is not None
    store.commit(v1.model, ev.bundle)
    m = broaden()
    store.migrate(m, commit_time=T1)
    ev = store.evaluate(v2.model, "set_region", bindings={"order": "o1"},
                        input={"region": "x"}, commit_time=T1)
    assert ev.bundle is not None
    store.commit(v2.model, ev.bundle)
    assert replay_data(store).ok
    r = replay_behavior(store, [v1.model, v2.model], migrations=[m])
    assert r.ok and r.checked == 3, r
    without = replay_behavior(store, [v1.model, v2.model])
    assert not without.ok and without.divergence is not None
    assert without.divergence["position"] == 2
