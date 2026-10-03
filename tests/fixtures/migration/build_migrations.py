"""Builds the migration fixtures in tests/fixtures/migration (feature 009).

Like the wire fixtures, everything is written here as Python data, independent of the DSL under
test. The only engine call is the SchemaHash of each module (migration documents name their
source and target schema by hash). Run from the repository root, with the binding installed:

    python tests/fixtures/migration/build_migrations.py

Layout:
- `modules/<name>.json`: wire IR modules, the schema generations of the cultures domain;
- `valid/<case>.json` + `<case>.expected.json`: migrations that must be admitted, with the
  modules they relate and the expected reviewable summary;
- `invalid/<case>.json` + `<case>.expected.json`: migrations that must be refused, with the
  modules they are admitted against and the expected error codes.

Outputs are canonical JSON (sorted keys, compact) and are committed. Review diffs by hand.
"""

from __future__ import annotations

import copy
import json
import pathlib
from typing import Any

ROOT = pathlib.Path(__file__).resolve().parent
J = dict[str, Any]


def dump(path: pathlib.Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False),
        encoding="utf-8",
    )


def loc(file: str, line: int) -> J:
    return {"file": file, "line": line}


# --- wire builders ----------------------------------------------------------------------------

def lit(ty: J, value: object, at: J) -> J:
    return {"op": "lit", "type": ty, "value": value, "loc": at}


def fld(param: str, field: str, at: J) -> J:
    return {"op": "field", "param": param, "field": field, "loc": at}


def op(name: str, *args: J, at: J) -> J:
    return {"op": name, "args": list(args), "loc": at}


BOOL = {"t": "bool"}
INT = {"t": "int"}
DEC = {"t": "decimal"}
STR = {"t": "string"}


def enum(name: str, side: str | None = None) -> J:
    t: J = {"t": "enum", "name": name}
    if side:
        t["side"] = side
    return t


def nominal(name: str, side: str | None = None) -> J:
    t: J = {"t": "nominal", "name": name}
    if side:
        t["side"] = side
    return t


def opt(of: J) -> J:
    return {"t": "option", "of": of}


def ent(name: str) -> J:
    return {"t": "entity", "name": name}


def ref(entity: str) -> J:
    return {"t": "ref", "entity": entity}


def field(name: str, ty: J, at: J) -> J:
    return {"name": name, "type": ty, "loc": at}


# --- the cultures domain: three schema generations ---------------------------------------------

def v1() -> J:
    f = "cultures_v1.py"
    return {
        "ir_version": "0.6",
        "enums": [
            {"name": "MediumKind", "values": ["MS", "WPM"], "loc": loc(f, 3)},
            {"name": "Status", "values": ["ACTIVE", "DEAD"], "loc": loc(f, 4)},
        ],
        "nominals": [
            {"name": "Money", "underlying": DEC, "ops": ["add", "order"], "scale": 2,
             "loc": loc(f, 6)},
            {"name": "Fine", "underlying": DEC, "ops": ["add", "order"], "scale": 4,
             "loc": loc(f, 7)},
        ],
        "entities": [
            {"name": "Customer", "loc": loc(f, 10), "fields": [
                field("name", STR, loc(f, 11)),
                field("email", STR, loc(f, 12)),
            ]},
            {"name": "Culture", "loc": loc(f, 14), "fields": [
                field("medium", enum("MediumKind"), loc(f, 15)),
                field("status", enum("Status"), loc(f, 16)),
                field("ph", INT, loc(f, 17)),
                field("legacy_code", STR, loc(f, 18)),
                field("price", nominal("Money"), loc(f, 19)),
                field("fee", nominal("Fine"), loc(f, 20)),
            ]},
            {"name": "Order", "loc": loc(f, 22), "fields": [
                field("customer", ref("Customer"), loc(f, 23)),
                field("region", opt(STR), loc(f, 24)),
                field("qty", INT, loc(f, 25)),
            ]},
            {"name": "AuditNote", "loc": loc(f, 27), "fields": [
                field("text", STR, loc(f, 28)),
            ]},
        ],
        "derived": [],
        "invariants": [],
        "constraints": [
            {"name": "ph_not_negative", "entity": "Culture", "param": "c", "loc": loc(f, 31),
             "body": op("ge", fld("c", "ph", loc(f, 32)), lit(INT, 0, loc(f, 32)), at=loc(f, 32))},
        ],
        "actions": [
            {"name": "set_region", "loc": loc(f, 35),
             "params": [{"name": "order", "role": "state", "type": ent("Order")},
                        {"name": "region", "role": "input", "type": STR}],
             "preconditions": [], "postconditions": [],
             "effects": [{"target": {"param": "order", "field": "region"}, "loc": loc(f, 36),
                          "value": {"op": "param", "param": "region", "loc": loc(f, 36)}}]},
            {"name": "kill", "loc": loc(f, 38),
             "params": [{"name": "culture", "role": "state", "type": ent("Culture")}],
             "preconditions": [], "postconditions": [],
             "effects": [{"target": {"param": "culture", "field": "status"}, "loc": loc(f, 39),
                          "value": lit(enum("Status"), "DEAD", loc(f, 39))}]},
            {"name": "register_customer", "loc": loc(f, 41),
             "params": [{"name": "customer_id", "role": "input",
                         "type": {"t": "id", "entity": "Customer"}},
                        {"name": "name", "role": "input", "type": STR},
                        {"name": "email", "role": "input", "type": STR}],
             "preconditions": [], "postconditions": [],
             "effects": [{"create": "Customer", "loc": loc(f, 42),
                          "id": {"op": "param", "param": "customer_id", "loc": loc(f, 42)},
                          "fields": {
                              "name": {"op": "param", "param": "name", "loc": loc(f, 42)},
                              "email": {"op": "param", "param": "email", "loc": loc(f, 42)}}}]},
            {"name": "forget_customer", "loc": loc(f, 44),
             "params": [{"name": "customer", "role": "state", "type": ent("Customer")}],
             "preconditions": [], "postconditions": [],
             "effects": [{"remove": "customer", "loc": loc(f, 45)}]},
        ],
    }


def v2() -> J:
    """V1 → V2: `MediumKind` gains `B5`; `medium` is renamed `medium_type`; `ph` becomes a decimal;
    `notes` is a new optional field; `legacy_code` is dropped; `Money` widens to scale 4 and `Fine`
    narrows to scale 2; `Order.customer` is renamed `buyer`; Customer's fields are reordered;
    `AuditNote` is retired."""
    f = "cultures_v2.py"
    w = v1()
    w["enums"][0] = {"name": "MediumKind", "values": ["MS", "WPM", "B5"], "loc": loc(f, 3)}
    w["nominals"] = [
        {"name": "Money", "underlying": DEC, "ops": ["add", "order"], "scale": 4, "loc": loc(f, 6)},
        {"name": "Fine", "underlying": DEC, "ops": ["add", "order"], "scale": 2, "loc": loc(f, 7)},
    ]
    w["entities"] = [
        {"name": "Customer", "loc": loc(f, 10), "fields": [
            field("email", STR, loc(f, 11)),
            field("name", STR, loc(f, 12)),
        ]},
        {"name": "Culture", "loc": loc(f, 14), "fields": [
            field("medium_type", enum("MediumKind"), loc(f, 15)),
            field("status", enum("Status"), loc(f, 16)),
            field("ph", DEC, loc(f, 17)),
            field("notes", opt(STR), loc(f, 18)),
            field("price", nominal("Money"), loc(f, 19)),
            field("fee", nominal("Fine"), loc(f, 20)),
        ]},
        {"name": "Order", "loc": loc(f, 22), "fields": [
            field("buyer", ref("Customer"), loc(f, 23)),
            field("region", opt(STR), loc(f, 24)),
            field("qty", INT, loc(f, 25)),
        ]},
    ]
    w["constraints"] = [
        {"name": "ph_not_negative", "entity": "Culture", "param": "c", "loc": loc(f, 31),
         "body": op("ge", fld("c", "ph", loc(f, 32)), lit(DEC, "0", loc(f, 32)), at=loc(f, 32))},
    ]
    return w


def v3() -> J:
    """V2 → V3: `Order.region` becomes required."""
    f = "cultures_v3.py"
    w = v2()
    w["entities"][2] = {"name": "Order", "loc": loc(f, 22), "fields": [
        field("buyer", ref("Customer"), loc(f, 23)),
        field("region", STR, loc(f, 24)),
        field("qty", INT, loc(f, 25)),
    ]}
    return w


def v4() -> J:
    """V3 → V4: customers gain an optional `vip` flag."""
    f = "cultures_v4.py"
    w = v3()
    w["entities"][0] = {"name": "Customer", "loc": loc(f, 10), "fields": [
        field("email", STR, loc(f, 11)),
        field("name", STR, loc(f, 12)),
        field("vip", opt(BOOL), loc(f, 13)),
    ]}
    for a in w["actions"]:
        if a["name"] == "register_customer":
            a["effects"][0]["fields"]["vip"] = lit(opt(BOOL), None, loc(f, 42))
    return w


MODULES = {"cultures_v1": v1(), "cultures_v2": v2(), "cultures_v3": v3(), "cultures_v4": v4()}


def schema_hashes() -> dict[str, str]:
    from behavior import _engine

    out = {}
    for name, w in MODULES.items():
        m, report = _engine.Module.from_wire(json.dumps(w))
        if m is None:
            raise SystemExit(f"{name} is not admissible: {report}")
        out[name] = str(m.schema_hash)
    return out


# --- migrations ------------------------------------------------------------------------------

def copy_of(f: str) -> J:
    return {"copy": f}


def enum_map(arg: J, to: J, mapping: list[list[str]], at: J, strict: bool = False) -> J:
    return {"op": "strict_enum_map" if strict else "enum_map", "args": [arg], "to": to,
            "mapping": mapping, "loc": at}


def rescale(arg: J, nominal_name: str, rounding: str, at: J, side: str | None = None) -> J:
    e: J = {"op": "rescale", "nominal": nominal_name, "rounding": rounding, "args": [arg], "loc": at}
    if side:
        e["side"] = side
    return e


def v1_to_v2(h: dict[str, str]) -> J:
    f = "migrations.py"
    old = lambda field_, line: fld("old", field_, loc(f, line))  # noqa: E731
    return {
        "migration_ir": "0.1",
        "name": "cultures_v1_to_v2",
        "source": h["cultures_v1"],
        "target": h["cultures_v2"],
        "constants": [],
        "requirements": [],
        "transforms": [
            {"entity": "Culture", "loc": loc(f, 10), "drops": ["legacy_code"], "fields": {
                "medium_type": enum_map(old("medium", 11), enum("MediumKind", "target"),
                                        [["MS", "MS"], ["WPM", "WPM"]], loc(f, 11)),
                "ph": old("ph", 12),
                "notes": lit(opt(STR), None, loc(f, 13)),
                "price": rescale(op("unwrap", old("price", 14), at=loc(f, 14)), "Money",
                                 "half_even", loc(f, 14), side="target"),
                "fee": rescale(op("unwrap", old("fee", 15), at=loc(f, 15)), "Fine",
                               "half_even", loc(f, 15), side="target"),
            }},
            {"entity": "Order", "loc": loc(f, 17), "drops": [], "fields": {
                "buyer": old("customer", 18),
            }},
        ],
        "retire": ["AuditNote"],
    }


V1_TO_V2_SUMMARY = {
    "types": {
        "Culture": {"copied": ["id", "status"], "transformed": ["fee", "medium_type", "ph", "price"],
                    "new": ["notes"], "dropped": ["legacy_code"]},
        "Customer": {"copied": ["email", "id", "name"], "transformed": [], "new": [],
                     "dropped": []},
        "Order": {"copied": ["id", "qty", "region"], "transformed": ["buyer"], "new": [],
                  "dropped": []},
    },
    "retired": ["AuditNote"],
}


def v2_to_v3(h: dict[str, str]) -> J:
    f = "migrations.py"
    return {
        "migration_ir": "0.1",
        "name": "region_required",
        "source": h["cultures_v2"],
        "target": h["cultures_v3"],
        "constants": [],
        "requirements": [{
            "name": "every_order_has_region", "loc": loc(f, 30),
            "body": {"op": "all", "args": [{"op": "select", "entity": "Order", "loc": loc(f, 30)}],
                     "param": "o", "loc": loc(f, 30),
                     "body": op("is_some", fld("o", "region", loc(f, 30)), at=loc(f, 30))},
        }],
        "transforms": [
            {"entity": "Order", "loc": loc(f, 32), "drops": [], "fields": {
                "region": op("strict_unwrap", fld("old", "region", loc(f, 33)), at=loc(f, 33)),
            }},
        ],
        "retire": [],
    }


V2_TO_V3_SUMMARY = {
    "types": {
        "Order": {"copied": ["buyer", "id", "qty"], "transformed": ["region"], "new": [],
                  "dropped": []},
    },
    "retired": [],
}


def v3_to_v4(h: dict[str, str]) -> J:
    f = "migrations.py"
    return {
        "migration_ir": "0.1",
        "name": "customers_vip",
        "source": h["cultures_v3"],
        "target": h["cultures_v4"],
        "constants": [],
        "requirements": [],
        "transforms": [
            {"entity": "Customer", "loc": loc(f, 40), "drops": [], "fields": {
                "vip": lit(opt(BOOL), None, loc(f, 41)),
            }},
        ],
        "retire": [],
    }


V3_TO_V4_SUMMARY = {
    "types": {
        "Customer": {"copied": ["email", "id", "name"], "transformed": [], "new": ["vip"],
                     "dropped": []},
    },
    "retired": [],
}


def invalid_cases(h: dict[str, str]) -> dict[str, tuple[J, str, str, list[str]]]:
    """case → (document, source module, target module, expected error codes)."""
    base = v1_to_v2(h)
    culture = 0
    out: dict[str, tuple[J, str, str, list[str]]] = {}

    d = copy.deepcopy(base)
    del d["transforms"][culture]["fields"]["notes"]
    out["missing_field"] = (d, "cultures_v1", "cultures_v2", ["MISSING_MIGRATION_FIELD"])

    d = copy.deepcopy(base)
    d["transforms"][culture]["drops"] = []
    out["unacknowledged_drop"] = (d, "cultures_v1", "cultures_v2", ["UNACKNOWLEDGED_FIELD_DROP"])

    d = copy.deepcopy(base)
    d["transforms"][culture]["fields"]["medium_type"]["mapping"] = [["MS", "MS"]]
    out["partial_enum_map"] = (d, "cultures_v1", "cultures_v2", ["UNMAPPED_ENUM_VALUE"])

    d = copy.deepcopy(base)
    d["transforms"][culture]["fields"]["medium_type"] = fld("old", "medium", loc("migrations.py", 11))
    out["changed_enum_assigned"] = (d, "cultures_v1", "cultures_v2", ["MIGRATION_TYPE_MISMATCH"])

    d = copy.deepcopy(base)
    d["transforms"][culture]["fields"]["fee"] = op(
        "unwrap", fld("old", "fee", loc("migrations.py", 15)), at=loc("migrations.py", 15))
    out["lossy_without_rounding"] = (d, "cultures_v1", "cultures_v2", ["MIGRATION_TYPE_MISMATCH"])

    d = copy.deepcopy(base)
    out["wrong_source_module"] = (d, "cultures_v2", "cultures_v2", ["MIGRATION_SCHEMA_MISMATCH"])

    d = copy.deepcopy(base)
    d["target"] = d["source"]
    out["same_source_and_target"] = (d, "cultures_v1", "cultures_v1", ["MIGRATION_SCHEMA_MISMATCH"])

    d = copy.deepcopy(base)
    d["retire"] = []
    out["retired_type_not_declared"] = (d, "cultures_v1", "cultures_v2", ["MISSING_RETIREMENT"])

    d = copy.deepcopy(base)
    d["transforms"][culture]["fields"]["id"] = lit({"t": "id", "entity": "Culture"}, "x",
                                                   loc("migrations.py", 16))
    out["identity_assigned"] = (d, "cultures_v1", "cultures_v2", ["MIGRATION_TYPE_MISMATCH"])
    return out


def main() -> None:
    for name, w in MODULES.items():
        dump(ROOT / "modules" / f"{name}.json", w)
    h = schema_hashes()
    valid = {
        "cultures_v1_to_v2": (v1_to_v2(h), "cultures_v1", "cultures_v2", V1_TO_V2_SUMMARY),
        "region_required": (v2_to_v3(h), "cultures_v2", "cultures_v3", V2_TO_V3_SUMMARY),
        "customers_vip": (v3_to_v4(h), "cultures_v3", "cultures_v4", V3_TO_V4_SUMMARY),
    }
    for case, (doc, s, t, summary) in valid.items():
        dump(ROOT / "valid" / f"{case}.json", doc)
        dump(ROOT / "valid" / f"{case}.expected.json",
             {"source": s, "target": t, "summary": summary})
    for case, (doc, s, t, codes) in invalid_cases(h).items():
        dump(ROOT / "invalid" / f"{case}.json", doc)
        dump(ROOT / "invalid" / f"{case}.expected.json",
             {"source": s, "target": t, "errors": [{"code": c} for c in codes]})


if __name__ == "__main__":
    main()
