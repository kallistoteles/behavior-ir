"""Builds the hand-authored wire IR fixtures in tests/fixtures/wire/{valid,invalid}.

The fixtures are written here as Python data (not with the behavior DSL) so that they are
independent of the DSL under test. Run from the repository root:

    python tests/fixtures/wire/build_fixtures.py

Outputs are canonical JSON (sorted keys, compact) and are committed; this script only exists
to keep source locations and repetitive structure consistent. Review diffs by hand.
"""

from __future__ import annotations

import copy
import json
import pathlib

ROOT = pathlib.Path(__file__).resolve().parent


def dump(path: pathlib.Path, value: object) -> None:
    path.write_text(
        json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False),
        encoding="utf-8",
    )


def loc(file: str, line: int) -> dict[str, object]:
    return {"file": file, "line": line}


# --- expression builders ---------------------------------------------------------------

def lit(ty: dict[str, object], value: object, at: dict[str, object]) -> dict[str, object]:
    return {"op": "lit", "type": ty, "value": value, "loc": at}


def fld(param: str, field: str, at: dict[str, object]) -> dict[str, object]:
    return {"op": "field", "param": param, "field": field, "loc": at}


def par(param: str, at: dict[str, object]) -> dict[str, object]:
    return {"op": "param", "param": param, "loc": at}


def der(name: str, args: list[str], at: dict[str, object]) -> dict[str, object]:
    return {"op": "derived", "name": name, "args": args, "loc": at}


def op(name: str, *args: dict[str, object], at: dict[str, object]) -> dict[str, object]:
    return {"op": name, "args": list(args), "loc": at}


BOOL = {"t": "bool"}
INT = {"t": "int"}
DEC = {"t": "decimal"}
STR = {"t": "string"}
MONEY = {"t": "nominal", "name": "Money"}
STATUS = {"t": "enum", "name": "InvoiceStatus"}


def ent(name: str) -> dict[str, object]:
    return {"t": "entity", "name": name}


def idt(entity: str) -> dict[str, object]:
    return {"t": "id", "entity": entity}


def opt(of: dict[str, object]) -> dict[str, object]:
    return {"t": "option", "of": of}


def param(name: str, ty: dict[str, object], role: str | None = None) -> dict[str, object]:
    p: dict[str, object] = {"name": name, "type": ty}
    if role is not None:
        p["role"] = role
    return p


def field_decl(name: str, ty: dict[str, object], at: dict[str, object]) -> dict[str, object]:
    return {"name": name, "type": ty, "loc": at}


def module(**parts: list[dict[str, object]]) -> dict[str, object]:
    m: dict[str, object] = {
        "ir_version": "0.1",
        "enums": [],
        "nominals": [],
        "entities": [],
        "derived": [],
        "invariants": [],
        "actions": [],
    }
    m.update(parts)
    return m


# --- invoice -----------------------------------------------------------------------------

F = "invoice.py"


def invoice() -> dict[str, object]:
    return module(
        nominals=[{"name": "Money", "underlying": DEC, "ops": ["add", "order", "ratio", "scale"],
                   "loc": loc(F, 3)}],
        enums=[{"name": "InvoiceStatus", "values": ["pending", "approved"], "loc": loc(F, 5)}],
        entities=[
            {"name": "User", "loc": loc(F, 10), "fields": [
                field_decl("role", STR, loc(F, 11)),
                field_decl("approval_limit", MONEY, loc(F, 12)),
            ]},
            {"name": "Invoice", "loc": loc(F, 15), "fields": [
                field_decl("amount", MONEY, loc(F, 16)),
                field_decl("status", STATUS, loc(F, 17)),
                field_decl("approved_by", opt(idt("User")), loc(F, 18)),
            ]},
        ],
        invariants=[
            {"name": "non_negative_amount", "entity": "Invoice", "param": "invoice",
             "body": op("ge", fld("invoice", "amount", loc(F, 22)),
                        lit(MONEY, "0", loc(F, 22)), at=loc(F, 22)),
             "loc": loc(F, 21)},
        ],
        actions=[
            {"name": "approve_invoice", "loc": loc(F, 25),
             "params": [param("invoice", ent("Invoice"), "state"),
                        param("actor", ent("User"), "context")],
             "preconditions": [
                 {"expr": op("eq", fld("invoice", "status", loc(F, 27)),
                             lit(STATUS, "pending", loc(F, 27)), at=loc(F, 27)),
                  "loc": loc(F, 27)},
                 {"expr": op("eq", fld("actor", "role", loc(F, 28)),
                             lit(STR, "manager", loc(F, 28)), at=loc(F, 28)),
                  "loc": loc(F, 28)},
                 {"expr": op("le", fld("invoice", "amount", loc(F, 29)),
                             fld("actor", "approval_limit", loc(F, 29)), at=loc(F, 29)),
                  "loc": loc(F, 29)},
             ],
             "effects": [
                 {"target": {"param": "invoice", "field": "status"},
                  "value": lit(STATUS, "approved", loc(F, 30)), "loc": loc(F, 30)},
                 {"target": {"param": "invoice", "field": "approved_by"},
                  "value": fld("actor", "id", loc(F, 31)), "loc": loc(F, 31)},
             ],
             "postconditions": [
                 {"expr": op("eq", fld("invoice", "approved_by", loc(F, 32)),
                             fld("actor", "id", loc(F, 32)), at=loc(F, 32)),
                  "loc": loc(F, 32)},
             ]},
            {"name": "apply_discount", "loc": loc(F, 35),
             "params": [param("invoice", ent("Invoice"), "state"),
                        param("discount", MONEY, "input")],
             "preconditions": [
                 {"expr": op("eq", fld("invoice", "status", loc(F, 37)),
                             lit(STATUS, "pending", loc(F, 37)), at=loc(F, 37)),
                  "loc": loc(F, 37)},
             ],
             "effects": [
                 {"target": {"param": "invoice", "field": "amount"},
                  "value": op("sub", fld("invoice", "amount", loc(F, 38)),
                              par("discount", loc(F, 38)), at=loc(F, 38)),
                  "loc": loc(F, 38)},
             ],
             "postconditions": []},
        ],
    )


# --- project margin ------------------------------------------------------------------------

P = "project_margin.py"


def project_margin() -> dict[str, object]:
    return module(
        nominals=[{"name": "Money", "underlying": DEC, "ops": ["add", "order", "ratio", "scale"],
                   "loc": loc(P, 3)}],
        entities=[
            {"name": "Project", "loc": loc(P, 6), "fields": [
                field_decl("revenue", MONEY, loc(P, 7)),
                field_decl("cost", MONEY, loc(P, 8)),
                field_decl("flagged", BOOL, loc(P, 9)),
            ]},
        ],
        derived=[
            {"name": "margin", "kind": "derived", "loc": loc(P, 12),
             "params": [param("project", ent("Project"))],
             "body": op("div",
                        op("sub", fld("project", "revenue", loc(P, 14)),
                           fld("project", "cost", loc(P, 14)), at=loc(P, 14)),
                        fld("project", "revenue", loc(P, 14)), at=loc(P, 14))},
            {"name": "high_risk", "kind": "rule", "loc": loc(P, 17),
             "params": [param("project", ent("Project"))],
             "body": op("lt", der("margin", ["project"], loc(P, 19)),
                        lit(DEC, "0.05", loc(P, 19)), at=loc(P, 19))},
        ],
        actions=[
            {"name": "flag_project", "loc": loc(P, 22),
             "params": [param("project", ent("Project"), "state")],
             "preconditions": [
                 {"expr": op("and",
                             op("ne", fld("project", "revenue", loc(P, 24)),
                                lit(MONEY, "0", loc(P, 24)), at=loc(P, 24)),
                             der("high_risk", ["project"], loc(P, 24)), at=loc(P, 24)),
                  "loc": loc(P, 24)},
             ],
             "effects": [
                 {"target": {"param": "project", "field": "flagged"},
                  "value": lit(BOOL, True, loc(P, 25)), "loc": loc(P, 25)},
             ],
             "postconditions": []},
            {"name": "flag_project_unguarded", "loc": loc(P, 28),
             "params": [param("project", ent("Project"), "state")],
             "preconditions": [
                 {"expr": der("high_risk", ["project"], loc(P, 30)), "loc": loc(P, 30)},
             ],
             "effects": [
                 {"target": {"param": "project", "field": "flagged"},
                  "value": lit(BOOL, True, loc(P, 31)), "loc": loc(P, 31)},
             ],
             "postconditions": []},
        ],
    )


# --- invalid cases --------------------------------------------------------------------------

def err(code: str, file: str | None = None, line: int | None = None) -> dict[str, object]:
    e: dict[str, object] = {"code": code}
    if file is not None:
        e["loc"] = loc(file, line or 0)
    return e


def invalid_cases() -> dict[str, tuple[dict[str, object], list[dict[str, object]]]]:
    cases: dict[str, tuple[dict[str, object], list[dict[str, object]]]] = {}
    base = invoice

    m = base()
    m["unexpected"] = True
    cases["unknown_key"] = (m, [err("DECODE_ERROR")])

    m = base()
    m["ir_version"] = "0.9"
    cases["ir_version_0_9"] = (m, [err("UNSUPPORTED_IR_VERSION")])

    m = base()
    m["actions"][0]["params"][1]["type"] = ent("Customer")  # type: ignore[index]
    cases["unknown_entity"] = (m, [err("UNKNOWN_ENTITY", F, 25)])

    m = base()
    m["entities"][0]["fields"][1]["type"] = {"t": "nominal", "name": "Currency"}  # type: ignore[index]
    cases["unknown_type"] = (m, [err("UNKNOWN_TYPE", F, 12)])

    m = base()
    m["actions"][0]["preconditions"][1]["expr"]["args"][0]["field"] = "title"  # type: ignore[index]
    cases["unknown_field"] = (m, [err("UNKNOWN_FIELD", F, 28)])

    m = project_margin()
    m["actions"][1]["preconditions"][0]["expr"]["name"] = "very_high_risk"  # type: ignore[index]
    cases["unknown_derived"] = (m, [err("UNKNOWN_DERIVED", P, 30)])

    m = base()
    m["actions"][0]["preconditions"][1]["expr"]["args"][0]["param"] = "approver"  # type: ignore[index]
    cases["unknown_param"] = (m, [err("UNKNOWN_PARAM", F, 28)])

    m = base()
    dup = copy.deepcopy(m["entities"][0])  # type: ignore[index]
    dup["loc"] = loc(F, 40)
    m["entities"].append(dup)  # type: ignore[union-attr]
    cases["duplicate_entity_name"] = (m, [err("DUPLICATE_NAME", F, 40)])

    m = base()
    m["entities"][0]["fields"].append(field_decl("id", STR, loc(F, 13)))  # type: ignore[index]
    cases["field_named_id"] = (m, [err("RESERVED_NAME", F, 13)])

    m = base()
    m["actions"][1]["effects"][0]["value"]["args"][1] = lit(DEC, "10", loc(F, 38))  # type: ignore[index]
    cases["money_plus_decimal"] = (m, [err("TYPE_MISMATCH", F, 38)])

    m = base()
    m["entities"].append({"name": "Project", "loc": loc(F, 42), "fields": [  # type: ignore[union-attr]
        field_decl("name", STR, loc(F, 43))]})
    m["actions"][0]["params"].append(param("project", ent("Project"), "context"))  # type: ignore[index]
    m["actions"][0]["postconditions"][0]["expr"]["args"][1] = fld("project", "id", loc(F, 32))  # type: ignore[index]
    cases["id_user_vs_id_project"] = (m, [err("TYPE_MISMATCH", F, 32)])

    m = base()
    m["actions"][0]["preconditions"].append(  # type: ignore[index]
        {"expr": op("le", fld("invoice", "approved_by", loc(F, 33)),
                    fld("actor", "id", loc(F, 33)), at=loc(F, 33)), "loc": loc(F, 33)})
    cases["option_used_as_value"] = (m, [err("TYPE_MISMATCH", F, 33)])

    m = project_margin()
    m["actions"][1]["preconditions"][0]["expr"]["args"] = ["project", "project"]  # type: ignore[index]
    cases["derived_arity"] = (m, [err("ARITY_MISMATCH", P, 30)])

    m = base()
    m["actions"][0]["preconditions"][0]["expr"] = fld("invoice", "amount", loc(F, 27))  # type: ignore[index]
    cases["precondition_not_bool"] = (m, [err("NOT_BOOLEAN", F, 27)])

    m = base()
    m["nominals"][0]["ops"] = ["add", "ratio", "scale"]  # type: ignore[index]
    cases["order_without_op"] = (m, [err("OP_NOT_ALLOWED", F, 22), err("OP_NOT_ALLOWED", F, 29)])

    m = module(
        entities=[{"name": "Node", "loc": loc("cycles.py", 3), "fields": [
            field_decl("weight", INT, loc("cycles.py", 4))]}],
        derived=[
            {"name": "a", "kind": "derived", "loc": loc("cycles.py", 7),
             "params": [param("n", ent("Node"))],
             "body": op("add", der("b", ["n"], loc("cycles.py", 8)),
                        lit(INT, 1, loc("cycles.py", 8)), at=loc("cycles.py", 8))},
            {"name": "b", "kind": "derived", "loc": loc("cycles.py", 11),
             "params": [param("n", ent("Node"))],
             "body": op("add", der("c", ["n"], loc("cycles.py", 12)),
                        lit(INT, 1, loc("cycles.py", 12)), at=loc("cycles.py", 12))},
            {"name": "c", "kind": "derived", "loc": loc("cycles.py", 15),
             "params": [param("n", ent("Node"))],
             "body": op("add", der("a", ["n"], loc("cycles.py", 16)),
                        fld("n", "weight", loc("cycles.py", 16)), at=loc("cycles.py", 16))},
        ],
    )
    cases["cycle_a_b_c"] = (m, [err("CYCLE", "cycles.py", 7)])

    m = base()
    m["actions"][0]["effects"].append(  # type: ignore[index]
        {"target": {"param": "invoice", "field": "status"},
         "value": lit(STATUS, "pending", loc(F, 34)), "loc": loc(F, 34)})
    cases["duplicate_assignment"] = (m, [err("DUPLICATE_ASSIGNMENT", F, 34)])

    m = base()
    m["actions"][0]["effects"].append(  # type: ignore[index]
        {"target": {"param": "actor", "field": "role"},
         "value": lit(STR, "admin", loc(F, 34)), "loc": loc(F, 34)})
    cases["effect_on_context"] = (m, [err("EFFECT_ON_READONLY", F, 34)])

    m = base()
    m["actions"][0]["effects"].append(  # type: ignore[index]
        {"target": {"param": "invoice", "field": "id"},
         "value": lit(idt("Invoice"), "x", loc(F, 34)), "loc": loc(F, 34)})
    cases["effect_on_id"] = (m, [err("RESERVED_NAME", F, 34)])

    m = base()
    m["actions"][0]["preconditions"][1]["expr"] = {  # type: ignore[index]
        "op": "in", "args": [fld("actor", "role", loc(F, 28))], "values": [], "loc": loc(F, 28)}
    cases["empty_in"] = (m, [err("EMPTY_IN", F, 28)])

    m = base()
    m["invariants"][0]["body"]["args"][1] = lit(MONEY, 0.0, loc(F, 22))  # type: ignore[index]
    cases["float_literal"] = (m, [err("INVALID_LITERAL", F, 22)])

    m = base()
    m["entities"][1]["fields"][2]["type"] = opt(opt(idt("User")))  # type: ignore[index]
    cases["nested_option"] = (m, [err("NESTED_OPTION", F, 18)])

    m = base()
    m["actions"][0]["preconditions"][1]["expr"]["args"][1] = lit(INT, 1, loc(F, 28))  # type: ignore[index]
    m["actions"][0]["preconditions"][0]["loc"] = loc("a_other.py", 5)  # type: ignore[index]
    m["actions"][0]["preconditions"][0]["expr"] = fld("invoice", "amount", loc("a_other.py", 5))  # type: ignore[index]
    m["actions"][1]["effects"][0]["value"]["args"][1] = lit(DEC, "1", loc(F, 38))  # type: ignore[index]
    cases["multiple_errors"] = (m, [
        err("NOT_BOOLEAN", "a_other.py", 5),
        err("TYPE_MISMATCH", F, 28),
        err("TYPE_MISMATCH", F, 38),
    ])
    return cases


# --- constraints (ir_version 0.2) -------------------------------------------------------------

C = "constraints.py"


def constraints_module() -> dict[str, object]:
    m = module(
        nominals=[{"name": "Money", "underlying": DEC, "ops": ["add", "order", "ratio", "scale"],
                   "loc": loc(C, 3)}],
        entities=[
            {"name": "Employee", "loc": loc(C, 6), "fields": [
                field_decl("role", STR, loc(C, 7)),
                field_decl("approval_limit", MONEY, loc(C, 8)),
            ]},
            {"name": "Account", "loc": loc(C, 11), "fields": [
                field_decl("balance", MONEY, loc(C, 12)),
            ]},
        ],
        actions=[
            {"name": "transfer", "loc": loc(C, 25),
             "params": [param("from_", ent("Account"), "state"), param("to", ent("Account"), "state"),
                        param("amount", MONEY, "input")],
             "preconditions": [
                 {"expr": op("gt", par("amount", loc(C, 27)), lit(MONEY, "0", loc(C, 27)), at=loc(C, 27)),
                  "loc": loc(C, 27)},
             ],
             "effects": [
                 {"target": {"param": "from_", "field": "balance"},
                  "value": op("sub", fld("from_", "balance", loc(C, 28)), par("amount", loc(C, 28)), at=loc(C, 28)),
                  "loc": loc(C, 28)},
                 {"target": {"param": "to", "field": "balance"},
                  "value": op("add", fld("to", "balance", loc(C, 29)), par("amount", loc(C, 29)), at=loc(C, 29)),
                  "loc": loc(C, 29)},
             ],
             "postconditions": []},
            {"name": "review", "loc": loc(C, 32),
             "params": [param("account", ent("Account"), "state"), param("actor", ent("Employee"), "context")],
             "preconditions": [
                 {"expr": op("eq", fld("actor", "role", loc(C, 34)), lit(STR, "manager", loc(C, 34)), at=loc(C, 34)),
                  "loc": loc(C, 34)},
             ],
             "effects": [], "postconditions": []},
            {"name": "assign", "loc": loc(C, 37),
             "params": [param("account", ent("Account"), "state"), param("approver", ent("Employee"), "input")],
             "preconditions": [
                 {"expr": op("ge", fld("approver", "approval_limit", loc(C, 39)),
                             fld("account", "balance", loc(C, 39)), at=loc(C, 39)),
                  "loc": loc(C, 39)},
             ],
             "effects": [], "postconditions": []},
        ],
    )
    m["ir_version"] = "0.2"
    m["constraints"] = [
        {"name": "non_negative_limit", "entity": "Employee", "param": "e",
         "body": op("ge", fld("e", "approval_limit", loc(C, 16)), lit(MONEY, "0", loc(C, 16)), at=loc(C, 16)),
         "loc": loc(C, 15)},
        {"name": "non_negative_balance", "entity": "Account", "param": "a",
         "body": op("ge", fld("a", "balance", loc(C, 21)), lit(MONEY, "0", loc(C, 21)), at=loc(C, 21)),
         "loc": loc(C, 20)},
    ]
    return m


def constraint_invalid_cases() -> dict[str, tuple[dict[str, object], list[dict[str, object]]]]:
    cases: dict[str, tuple[dict[str, object], list[dict[str, object]]]] = {}
    m = constraints_module()
    m["constraints"][0]["entity"] = "Manager"  # type: ignore[index]
    cases["constraint_unknown_entity"] = (m, [err("UNKNOWN_ENTITY", C, 15)])
    m = constraints_module()
    m["constraints"][0]["body"] = fld("e", "approval_limit", loc(C, 16))  # type: ignore[index]
    cases["constraint_not_bool"] = (m, [err("NOT_BOOLEAN", C, 16)])
    m = constraints_module()
    m["ir_version"] = "0.1"
    cases["constraints_in_0_1"] = (m, [err("DECODE_ERROR")])
    m = constraints_module()
    m["constraints"][0]["name"] = "transfer"  # type: ignore[index]
    cases["constraint_duplicate_name"] = (m, [err("DUPLICATE_NAME", C, 25)])
    return cases


def main() -> None:
    valid = ROOT / "valid"
    invalid = ROOT / "invalid"
    valid.mkdir(exist_ok=True)
    invalid.mkdir(exist_ok=True)
    dump(valid / "invoice.json", invoice())
    dump(valid / "project_margin.json", project_margin())
    dump(valid / "empty.json", module())
    dump(valid / "constraints.json", constraints_module())
    for name, (doc, errors) in {**invalid_cases(), **constraint_invalid_cases()}.items():
        dump(invalid / f"{name}.json", doc)
        dump(invalid / f"{name}.expected.json", {"errors": errors})


if __name__ == "__main__":
    main()
