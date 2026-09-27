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
        "ir_version": "0.4",
        "constraints": [],
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
                   "scale": 2, "loc": loc(F, 3)}],
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
                   "scale": 2, "loc": loc(C, 3)}],
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
    cases["constraints_in_0_1"] = (m, [err("UNSUPPORTED_IR_VERSION")])
    m = constraints_module()
    m["constraints"][0]["name"] = "transfer"  # type: ignore[index]
    cases["constraint_duplicate_name"] = (m, [err("DUPLICATE_NAME", C, 25)])
    return cases


# --- feature 003: fixed-scale decimals --------------------------------------------------------

X = "fixed_scale.py"
EXACT_MONEY = {"t": "exact", "name": "Money"}


def rescale(arg: dict[str, object], rounding: str, at: dict[str, object], nominal: str = "Money") -> dict[str, object]:
    return {"op": "rescale", "nominal": nominal, "rounding": rounding, "args": [arg], "loc": at}


def fixed_scale_module() -> dict[str, object]:
    amount = lambda line: fld("invoice", "amount", loc(X, line))  # noqa: E731
    m = module(
        nominals=[{"name": "Money", "underlying": DEC, "ops": ["add", "order", "ratio", "scale"],
                   "scale": 2, "loc": loc(X, 3)}],
        entities=[
            {"name": "Invoice", "loc": loc(X, 6), "fields": [
                field_decl("amount", MONEY, loc(X, 7)),
                field_decl("fee", MONEY, loc(X, 8)),
            ]},
            {"name": "Budget", "loc": loc(X, 11), "fields": [
                field_decl("limit", MONEY, loc(X, 12)),
                field_decl("spent", MONEY, loc(X, 13)),
            ]},
        ],
        derived=[
            {"name": "theoretical_fee", "kind": "derived", "loc": loc(X, 16), "type": EXACT_MONEY,
             "params": [param("invoice", ent("Invoice"))],
             "body": op("mul", amount(17), lit(DEC, "0.25", loc(X, 17)), at=loc(X, 17))},
            {"name": "fee_rounded", "kind": "derived", "loc": loc(X, 20), "type": MONEY,
             "params": [param("invoice", ent("Invoice"))],
             "body": rescale(op("mul", amount(21), lit(DEC, "0.25", loc(X, 21)), at=loc(X, 21)),
                             "half_even", loc(X, 21))},
        ],
        actions=[
            {"name": "charge", "loc": loc(X, 24), "params": [param("invoice", ent("Invoice"), "state")],
             "preconditions": [],
             "effects": [{"target": {"param": "invoice", "field": "fee"},
                          "value": rescale(op("mul", amount(25), lit(DEC, "0.25", loc(X, 25)), at=loc(X, 25)),
                                           "half_even", loc(X, 25)),
                          "loc": loc(X, 25)}],
             "postconditions": []},
            {"name": "split3", "loc": loc(X, 28), "params": [param("invoice", ent("Invoice"), "state")],
             "preconditions": [],
             "effects": [{"target": {"param": "invoice", "field": "fee"},
                          "value": rescale(op("mul", op("div", amount(29), lit(INT, 3, loc(X, 29)), at=loc(X, 29)),
                                              lit(INT, 2, loc(X, 29)), at=loc(X, 29)),
                                           "half_even", loc(X, 29)),
                          "loc": loc(X, 29)}],
             "postconditions": []},
            {"name": "split3_nested", "loc": loc(X, 32), "params": [param("invoice", ent("Invoice"), "state")],
             "preconditions": [],
             "effects": [{"target": {"param": "invoice", "field": "fee"},
                          "value": rescale(op("mul", rescale(op("div", amount(33), lit(INT, 3, loc(X, 33)), at=loc(X, 33)),
                                                             "half_even", loc(X, 33)),
                                              lit(INT, 2, loc(X, 33)), at=loc(X, 33)),
                                           "half_even", loc(X, 33)),
                          "loc": loc(X, 33)}],
             "postconditions": []},
            {"name": "check_margin", "loc": loc(X, 36),
             "params": [param("invoice", ent("Invoice"), "state"), param("budget", ent("Budget"), "state")],
             "preconditions": [{"expr": op("le", op("mul", amount(37), lit(DEC, "1.25", loc(X, 37)), at=loc(X, 37)),
                                           fld("budget", "limit", loc(X, 37)), at=loc(X, 37)),
                                "loc": loc(X, 37)}],
             "effects": [], "postconditions": []},
            {"name": "add_spent", "loc": loc(X, 40),
             "params": [param("invoice", ent("Invoice"), "state"), param("budget", ent("Budget"), "state")],
             "preconditions": [],
             "effects": [{"target": {"param": "budget", "field": "spent"},
                          "value": op("add", fld("budget", "spent", loc(X, 41)), amount(41), at=loc(X, 41)),
                          "loc": loc(X, 41)}],
             "postconditions": []},
            {"name": "triple", "loc": loc(X, 44), "params": [param("budget", ent("Budget"), "state")],
             "preconditions": [],
             "effects": [{"target": {"param": "budget", "field": "spent"},
                          "value": op("mul", fld("budget", "spent", loc(X, 45)), lit(INT, 3, loc(X, 45)), at=loc(X, 45)),
                          "loc": loc(X, 45)}],
             "postconditions": []},
        ],
    )
    inv = [param("invoice", ent("Invoice"), "state")]

    def fee_action(name: str, line: int, value: dict[str, object], extra: list[dict[str, object]] | None = None) -> dict[str, object]:
        return {"name": name, "loc": loc(X, line), "params": inv + (extra or []), "preconditions": [],
                "effects": [{"target": {"param": "invoice", "field": "fee"}, "value": value, "loc": loc(X, line + 1)}],
                "postconditions": []}

    m["derived"].append(  # type: ignore[attr-defined]
        {"name": "third", "kind": "derived", "loc": loc(X, 50), "params": [param("invoice", ent("Invoice"))],
         "body": op("div", lit(INT, 1, loc(X, 51)), lit(INT, 3, loc(X, 51)), at=loc(X, 51))})
    m["actions"] += [  # type: ignore[operator]
        {"name": "compare_fee", "loc": loc(X, 54), "params": inv,
         "preconditions": [{"expr": op("ge", der("theoretical_fee", ["invoice"], loc(X, 55)),
                                       der("fee_rounded", ["invoice"], loc(X, 55)), at=loc(X, 55)),
                            "loc": loc(X, 55)}],
         "effects": [], "postconditions": []},
        fee_action("per_unit", 58, rescale(op("div", amount(59), par("count", loc(X, 59)), at=loc(X, 59)),
                                           "half_even", loc(X, 59)),
                   [param("count", INT, "input")]),
        fee_action("scale_up", 62, rescale(op("mul", amount(63), lit(DEC, "1000", loc(X, 63)), at=loc(X, 63)),
                                           "half_even", loc(X, 63))),
        fee_action("third_exact", 66, rescale(op("mul", amount(67),
                                                 op("div", lit(INT, 1, loc(X, 67)), lit(INT, 3, loc(X, 67)), at=loc(X, 67)),
                                                 at=loc(X, 67)), "floor", loc(X, 67))),
        fee_action("third_via_derived", 70, rescale(op("mul", amount(71), der("third", ["invoice"], loc(X, 71)),
                                                       at=loc(X, 71)), "floor", loc(X, 71))),
    ]
    for i, mode in enumerate(["half_even", "half_up", "down", "up", "floor", "ceiling"]):
        line = 80 + 3 * i
        m["actions"].append(  # type: ignore[attr-defined]
            fee_action(f"round_{mode}", line,
                       rescale(op("div", amount(line + 1), lit(INT, 8, loc(X, line + 1)), at=loc(X, line + 1)),
                               mode, loc(X, line + 1))))
    return m


def fixed_scale_invalid_cases() -> dict[str, tuple[dict[str, object], list[dict[str, object]]]]:
    cases: dict[str, tuple[dict[str, object], list[dict[str, object]]]] = {}
    base = fixed_scale_module

    m = base()
    m["nominals"][0]["underlying"] = INT  # type: ignore[index]
    cases["scale_not_decimal"] = (m, [err("SCALE_NOT_DECIMAL", X, 3)])
    m = base()
    m["nominals"][0]["scale"] = 29  # type: ignore[index]
    cases["scale_out_of_range"] = (m, [err("SCALE_OUT_OF_RANGE", X, 3)])
    m = base()
    m["actions"][4]["effects"][0]["value"] = op(  # type: ignore[index]
        "add", fld("budget", "spent", loc(X, 41)), lit(MONEY, "0.005", loc(X, 41)), at=loc(X, 41))
    cases["off_grid_literal"] = (m, [err("OFF_GRID_LITERAL", X, 41)])
    m = base()
    m["actions"][0]["effects"][0]["value"] = {  # type: ignore[index]
        "op": "wrap", "nominal": "Money",
        "args": [op("mul", op("unwrap", fld("invoice", "amount", loc(X, 25)), at=loc(X, 25)),
                    lit(DEC, "0.25", loc(X, 25)), at=loc(X, 25))], "loc": loc(X, 25)}
    cases["lossy_wrap"] = (m, [err("LOSSY_CONVERSION", X, 25)])
    m = base()
    m["actions"][0]["effects"][0]["value"] = op(  # type: ignore[index]
        "mul", fld("invoice", "amount", loc(X, 25)), lit(DEC, "0.25", loc(X, 25)), at=loc(X, 25))
    cases["exact_stored"] = (m, [err("LOSSY_CONVERSION", X, 25)])
    m = base()
    m["actions"][3]["preconditions"][0]["expr"] = op(  # type: ignore[index]
        "gt", op("unwrap", op("mul", fld("invoice", "amount", loc(X, 37)), lit(DEC, "1.25", loc(X, 37)),
                              at=loc(X, 37)), at=loc(X, 37)),
        lit(DEC, "0", loc(X, 37)), at=loc(X, 37))
    cases["exact_unwrap"] = (m, [err("LOSSY_CONVERSION", X, 37)])
    m = base()
    m["actions"][0]["effects"][0]["value"]["rounding"] = "bankers"  # type: ignore[index]
    cases["unknown_rounding"] = (m, [err("UNKNOWN_ROUNDING", X, 25)])
    m = base()
    m["nominals"].append({"name": "Plain", "underlying": DEC, "ops": ["add", "scale"], "loc": loc(X, 4)})  # type: ignore[attr-defined]
    m["actions"][0]["effects"][0]["value"]["nominal"] = "Plain"  # type: ignore[index]
    cases["rescale_not_fixed_scale"] = (m, [err("EXACT_NOT_FIXED_SCALE", X, 25)])
    m = base()
    m["derived"][0]["type"] = MONEY  # type: ignore[index]
    m["actions"] = [a for a in m["actions"] if a["name"] != "compare_fee"]  # type: ignore[index,union-attr]
    cases["declared_type_mismatch"] = (m, [err("DECLARED_TYPE_MISMATCH", X, 16)])
    m = base()
    m["entities"][0]["fields"].append(field_decl("estimate", EXACT_MONEY, loc(X, 9)))  # type: ignore[index]
    cases["exact_field"] = (m, [err("EXACT_FIELD", X, 9)])
    m = base()
    m["nominals"] = [
        {"name": "SEK", "underlying": DEC, "ops": ["add", "order", "scale"], "scale": 2, "loc": loc(X, 3)},
        {"name": "JPY", "underlying": DEC, "ops": ["add", "order", "scale"], "scale": 0, "loc": loc(X, 4)},
    ]
    m["entities"] = [{"name": "Wallet", "loc": loc(X, 6), "fields": [
        field_decl("sek", {"t": "nominal", "name": "SEK"}, loc(X, 7)),
        field_decl("jpy", {"t": "nominal", "name": "JPY"}, loc(X, 8)),
    ]}]
    m["derived"] = [
        {"name": "mixed", "kind": "rule", "loc": loc(X, 10), "params": [param("w", ent("Wallet"))],
         "body": op("lt", fld("w", "sek", loc(X, 11)), fld("w", "jpy", loc(X, 11)), at=loc(X, 11))},
        {"name": "mixed_exact", "kind": "rule", "loc": loc(X, 13), "params": [param("w", ent("Wallet"))],
         "body": op("lt", op("mul", fld("w", "sek", loc(X, 14)), lit(DEC, "0.5", loc(X, 14)), at=loc(X, 14)),
                    op("mul", fld("w", "jpy", loc(X, 14)), lit(DEC, "0.5", loc(X, 14)), at=loc(X, 14)),
                    at=loc(X, 14))},
    ]
    m["actions"] = []
    cases["mixed_nominals"] = (m, [err("TYPE_MISMATCH", X, 11), err("TYPE_MISMATCH", X, 14)])
    m = base()
    m["ir_version"] = "0.3"
    cases["fixed_scale_in_0_3"] = (m, [err("UNSUPPORTED_IR_VERSION")])
    return cases


# --- feature 004: exact bound ---------------------------------------------------------------------

B = "bounds.py"


def bound_module(divisions: int) -> dict[str, object]:
    """`a / d / d / …` with `a` two-decimal Money and `d` a general decimal: each division adds
    94 numerator bits, so 5 divisions need 564 bits (> 511)."""
    body: dict[str, object] = fld("r", "a", loc(B, 10))
    for _ in range(divisions):
        body = op("div", body, fld("r", "d", loc(B, 10)), at=loc(B, 10))
    return module(
        nominals=[{"name": "Money", "underlying": DEC, "ops": ["add", "order", "ratio", "scale"],
                   "scale": 2, "loc": loc(B, 3)}],
        entities=[{"name": "Row", "loc": loc(B, 5), "fields": [
            field_decl("a", MONEY, loc(B, 6)), field_decl("d", DEC, loc(B, 7))]}],
        derived=[{"name": "shrink", "kind": "derived", "loc": loc(B, 9),
                  "params": [param("r", ent("Row"))], "body": body}],
        actions=[{"name": "check", "loc": loc(B, 12), "params": [param("r", ent("Row"), "state")],
                  "preconditions": [{"expr": op("gt", der("shrink", ["r"], loc(B, 13)),
                                                lit(MONEY, "0", loc(B, 13)), at=loc(B, 13)),
                                     "loc": loc(B, 13)}],
                  "effects": [], "postconditions": []}],
    )


# --- feature 004: exact ratios -----------------------------------------------------------------

E = "exact_closure.py"
EXACT = {"t": "exact"}


def exact_closure_module() -> dict[str, object]:
    sf = lambda f, line: fld("share", f, loc(E, line))  # noqa: E731
    shr = [param("share", ent("Share"), "state")]
    return module(
        nominals=[{"name": "Money", "underlying": DEC, "ops": ["add", "order", "ratio", "scale"],
                   "scale": 2, "loc": loc(E, 3)}],
        entities=[{"name": "Share", "loc": loc(E, 5), "fields": [
            field_decl("amount", MONEY, loc(E, 6)), field_decl("budget", MONEY, loc(E, 7)),
            field_decl("total", MONEY, loc(E, 8)), field_decl("part", MONEY, loc(E, 9))]}],
        derived=[{"name": "portion", "kind": "derived", "loc": loc(E, 11), "type": EXACT,
                  "params": [param("share", ent("Share"))],
                  "body": op("div", sf("amount", 12), sf("budget", 12), at=loc(E, 12))}],
        actions=[
            {"name": "show_portion", "loc": loc(E, 14), "params": shr,
             "preconditions": [{"expr": op("ge", der("portion", ["share"], loc(E, 15)),
                                           lit(DEC, "0", loc(E, 15)), at=loc(E, 15)), "loc": loc(E, 15)}],
             "effects": [], "postconditions": []},
            {"name": "scaled_share", "loc": loc(E, 17), "params": shr, "preconditions": [],
             "effects": [{"target": {"param": "share", "field": "part"},
                          "value": rescale(op("mul", op("div", sf("amount", 18), sf("budget", 18), at=loc(E, 18)),
                                              sf("total", 18), at=loc(E, 18)), "half_even", loc(E, 18)),
                          "loc": loc(E, 18)}],
             "postconditions": []},
            {"name": "share_threshold", "loc": loc(E, 20), "params": shr,
             "preconditions": [{"expr": op("le", op("div", sf("amount", 21), sf("budget", 21), at=loc(E, 21)),
                                           lit(DEC, "0.25", loc(E, 21)), at=loc(E, 21)), "loc": loc(E, 21)}],
             "effects": [], "postconditions": []},
        ],
    )


def exact_closure_invalid_cases() -> dict[str, tuple[dict[str, object], list[dict[str, object]]]]:
    cases: dict[str, tuple[dict[str, object], list[dict[str, object]]]] = {}
    m = exact_closure_module()
    m["actions"][1]["effects"][0]["value"] = op(  # type: ignore[index]
        "div", fld("share", "amount", loc(E, 18)), fld("share", "budget", loc(E, 18)), at=loc(E, 18))
    cases["ratio_stored"] = (m, [err("LOSSY_CONVERSION", E, 18)])
    m = exact_closure_module()
    m["actions"][2]["preconditions"][0]["expr"] = op(  # type: ignore[index]
        "le", op("add", op("div", fld("share", "amount", loc(E, 21)), fld("share", "budget", loc(E, 21)),
                           at=loc(E, 21)), fld("share", "amount", loc(E, 21)), at=loc(E, 21)),
        lit(DEC, "0.25", loc(E, 21)), at=loc(E, 21))
    cases["ratio_plus_amount"] = (m, [err("TYPE_MISMATCH", E, 21)])
    m = exact_closure_module()
    m["nominals"] = [
        {"name": "SEK", "underlying": DEC, "ops": ["add", "order", "ratio", "scale"], "scale": 2, "loc": loc(E, 3)},
        {"name": "JPY", "underlying": DEC, "ops": ["add", "order", "ratio", "scale"], "scale": 0, "loc": loc(E, 4)},
    ]
    m["entities"] = [{"name": "Wallet", "loc": loc(E, 5), "fields": [
        field_decl("sek", {"t": "nominal", "name": "SEK"}, loc(E, 6)),
        field_decl("jpy", {"t": "nominal", "name": "JPY"}, loc(E, 7))]}]
    m["derived"] = [{"name": "rate", "kind": "derived", "loc": loc(E, 11), "params": [param("w", ent("Wallet"))],
                     "body": op("div", fld("w", "sek", loc(E, 12)), fld("w", "jpy", loc(E, 12)), at=loc(E, 12))}]
    m["actions"] = []
    cases["exchange_rate"] = (m, [err("TYPE_MISMATCH", E, 12)])
    return cases


G = "decimal_store.py"


def decimal_store_module(target: str, value: dict[str, object]) -> dict[str, object]:
    """Unscaled `Money` and general decimals stored from arithmetic (feature 004, US4)."""
    return module(
        nominals=[{"name": "Money", "underlying": DEC, "ops": ["add", "order", "ratio", "scale"],
                   "loc": loc(G, 3)}],
        entities=[{"name": "Order", "loc": loc(G, 5), "fields": [
            field_decl("amount", MONEY, loc(G, 6)), field_decl("discount", MONEY, loc(G, 7)),
            field_decl("net", MONEY, loc(G, 8)), field_decl("x", DEC, loc(G, 9)),
            field_decl("y", DEC, loc(G, 10))]}],
        actions=[{"name": "apply", "loc": loc(G, 12), "params": [param("o", ent("Order"), "state")],
                  "preconditions": [], "postconditions": [],
                  "effects": [{"target": {"param": "o", "field": target}, "value": value,
                               "loc": loc(G, 13)}]}],
    )


def decimal_store_invalid_cases() -> dict[str, tuple[dict[str, object], list[dict[str, object]]]]:
    o = lambda f: fld("o", f, loc(G, 13))  # noqa: E731
    return {
        "decimal_sum_stored": (
            decimal_store_module("net", op("sub", o("amount"), o("discount"), at=loc(G, 13))),
            [err("LOSSY_CONVERSION", G, 13)]),
        "decimal_ratio_stored": (
            decimal_store_module("y", op("div", o("x"), lit(INT, 3, loc(G, 13)), at=loc(G, 13))),
            [err("LOSSY_CONVERSION", G, 13)]),
    }


L = "ledger.py"


def ledger_module() -> dict[str, object]:
    """Accounts with transfers, a no-op action and a freeze (feature 005 persistence fixtures)."""
    acc = lambda p, f, line: fld(p, f, loc(L, line))  # noqa: E731
    zero = lambda line: lit(MONEY, "0", loc(L, line))  # noqa: E731
    m = module(
        nominals=[{"name": "Money", "underlying": DEC, "ops": ["add", "order", "ratio", "scale"],
                   "scale": 2, "loc": loc(L, 3)}],
        entities=[{"name": "Account", "loc": loc(L, 6), "fields": [
            field_decl("active", BOOL, loc(L, 7)), field_decl("balance", MONEY, loc(L, 8))]}],
        derived=[{"name": "available", "kind": "derived", "loc": loc(L, 15),
                  "params": [param("account", ent("Account"))],
                  "body": acc("account", "balance", 16)}],
        actions=[
            {"name": "transfer", "loc": loc(L, 19),
             "params": [param("from_", ent("Account"), "state"), param("to", ent("Account"), "state"),
                        param("amount", MONEY, "input")],
             "preconditions": [
                 {"expr": op("gt", par("amount", loc(L, 21)), zero(21), at=loc(L, 21)), "loc": loc(L, 21)},
                 {"expr": {"op": "and", "args": [
                     acc("from_", "active", 22),
                     op("ge", der("available", ["from_"], loc(L, 22)), par("amount", loc(L, 22)), at=loc(L, 22))],
                     "loc": loc(L, 22)}, "loc": loc(L, 22)},
             ],
             "effects": [
                 {"target": {"param": "from_", "field": "balance"},
                  "value": op("sub", acc("from_", "balance", 23), par("amount", loc(L, 23)), at=loc(L, 23)),
                  "loc": loc(L, 23)},
                 {"target": {"param": "to", "field": "balance"},
                  "value": op("add", acc("to", "balance", 24), par("amount", loc(L, 24)), at=loc(L, 24)),
                  "loc": loc(L, 24)},
             ],
             "postconditions": []},
            {"name": "touch", "loc": loc(L, 27), "params": [param("account", ent("Account"), "state")],
             "preconditions": [],
             "effects": [{"target": {"param": "account", "field": "active"},
                          "value": acc("account", "active", 28), "loc": loc(L, 28)}],
             "postconditions": []},
            {"name": "freeze", "loc": loc(L, 31), "params": [param("account", ent("Account"), "state")],
             "preconditions": [],
             "effects": [{"target": {"param": "account", "field": "active"},
                          "value": lit(BOOL, False, loc(L, 32)), "loc": loc(L, 32)}],
             "postconditions": []},
        ],
    )
    m["constraints"] = [
        {"name": "non_negative_balance", "entity": "Account", "param": "a",
         "body": op("ge", acc("a", "balance", 12), zero(12), at=loc(L, 12)), "loc": loc(L, 11)},
    ]
    return m


A = "accounts.py"


def ref(entity: str) -> dict[str, object]:
    return {"t": "ref", "entity": entity}


def create(entity: str, id_: dict[str, object], fields: dict[str, dict[str, object]],
           at: dict[str, object]) -> dict[str, object]:
    return {"create": entity, "id": id_, "fields": fields, "loc": at}


def remove(p: str, at: dict[str, object]) -> dict[str, object]:
    return {"remove": p, "loc": at}


def cond(expr: dict[str, object]) -> dict[str, object]:
    return {"expr": expr, "loc": expr["loc"]}


def accounts_module() -> dict[str, object]:
    """Customers, accounts that reference them, and audit notes (feature 006 lifecycle fixtures)."""
    at = lambda line: loc(A, line)  # noqa: E731
    zero = lambda line: lit(MONEY, "0", at(line))  # noqa: E731
    f = lambda p, fl, line: fld(p, fl, at(line))  # noqa: E731
    p = lambda name, line: par(name, at(line))  # noqa: E731

    def action(name: str, line: int, params: list[dict[str, object]], effects: list[dict[str, object]],
               pre: list[dict[str, object]] | None = None,
               post: list[dict[str, object]] | None = None) -> dict[str, object]:
        return {"name": name, "loc": at(line), "params": params, "effects": effects,
                "preconditions": pre or [], "postconditions": post or []}

    def open_account(name: str, line: int, guarded: bool) -> dict[str, object]:
        pre = [cond(op("ge", p("initial", line + 1), zero(line + 1), at=at(line + 1)))] if guarded else []
        return action(
            name, line,
            [param("owner", ent("Customer"), "state"), param("account_id", idt("Account"), "input"),
             param("initial", MONEY, "input")],
            [create("Account", p("account_id", line + 2),
                    {"owner": f("owner", "id", line + 2), "balance": p("initial", line + 2)}, at(line + 2))],
            pre, [cond(op("exists", p("account_id", line + 3), at=at(line + 3)))])

    def remove_customer(name: str, line: int, guarded: bool) -> dict[str, object]:
        pre = ([cond(op("not", op("referenced", f("customer", "id", line + 1), at=at(line + 1)),
                        at=at(line + 1)))] if guarded else [])
        return action(name, line, [param("customer", ent("Customer"), "state")],
                      [remove("customer", at(line + 2))], pre)

    m = module(
        nominals=[{"name": "Money", "underlying": DEC, "ops": ["add", "order", "ratio", "scale"],
                   "scale": 2, "loc": at(3)}],
        entities=[
            {"name": "Customer", "loc": at(6), "fields": [field_decl("name", STR, at(7))]},
            {"name": "Account", "loc": at(10), "fields": [
                field_decl("owner", ref("Customer"), at(11)), field_decl("balance", MONEY, at(12))]},
            {"name": "AuditNote", "loc": at(15), "fields": [
                field_decl("about", idt("Customer"), at(16)), field_decl("text", STR, at(17))]},
        ],
        actions=[
            open_account("open_account", 24, True),
            open_account("open_account_unchecked", 30, False),
            action("register_customer", 36,
                   [param("customer_id", idt("Customer"), "input"), param("name", STR, "input")],
                   [create("Customer", p("customer_id", 37), {"name": p("name", 37)}, at(37))]),
            action("deposit", 40,
                   [param("account", ent("Account"), "state"), param("amount", MONEY, "input")],
                   [{"target": {"param": "account", "field": "balance"},
                     "value": op("add", f("account", "balance", 42), p("amount", 42), at=at(42)),
                     "loc": at(42)}],
                   [cond(op("gt", p("amount", 41), zero(41), at=at(41)))]),
            action("close_account", 45, [param("account", ent("Account"), "state")],
                   [remove("account", at(47))],
                   [cond(op("eq", f("account", "balance", 46), zero(46), at=at(46)))],
                   [cond(op("not", op("exists", f("account", "id", 48), at=at(48)), at=at(48)))]),
            remove_customer("remove_customer", 51, True),
            remove_customer("remove_customer_unchecked", 55, False),
            action("switch_and_remove", 59,
                   [param("account", ent("Account"), "state"), param("old", ent("Customer"), "state"),
                    param("new", ent("Customer"), "state")],
                   [{"target": {"param": "account", "field": "owner"}, "value": f("new", "id", 61),
                     "loc": at(61)},
                    remove("old", at(62))],
                   [cond(op("eq", f("account", "owner", 60), f("old", "id", 60), at=at(60)))]),
            action("check_exists", 65, [param("note", ent("AuditNote"), "state")], [],
                   [cond(op("exists", f("note", "about", 66), at=at(66)))]),
            action("create_twice", 69,
                   [param("owner", ent("Customer"), "state"), param("a", idt("Account"), "input"),
                    param("b", idt("Account"), "input")],
                   [create("Account", p("a", 70), {"owner": f("owner", "id", 70), "balance": zero(70)}, at(70)),
                    create("Account", p("b", 71), {"owner": f("owner", "id", 71), "balance": zero(71)}, at(71))]),
        ],
    )
    m["ir_version"] = "0.5"
    m["constraints"] = [
        {"name": "non_negative_balance", "entity": "Account", "param": "a",
         "body": op("ge", f("a", "balance", 20), zero(20), at=at(20)), "loc": at(19)},
    ]
    return m


def accounts_action(m: dict[str, object], name: str) -> dict[str, object]:
    return next(a for a in m["actions"] if a["name"] == name)  # type: ignore[union-attr,index]


def lifecycle_invalid_cases() -> dict[str, tuple[dict[str, object], list[dict[str, object]]]]:
    cases: dict[str, tuple[dict[str, object], list[dict[str, object]]]] = {}

    m = accounts_module()
    del accounts_action(m, "open_account")["effects"][0]["fields"]["balance"]  # type: ignore[index]
    cases["create_incomplete"] = (m, [err("CREATE_INCOMPLETE", A, 26)])

    m = accounts_module()
    accounts_action(m, "open_account")["effects"][0]["id"] = fld("owner", "id", loc(A, 26))  # type: ignore[index]
    cases["create_wrong_id_type"] = (m, [err("TYPE_MISMATCH", A, 26)])

    m = accounts_module()
    m["ir_version"] = "0.4"
    cases["lifecycle_in_0_4"] = (m, [err("UNSUPPORTED_IR_VERSION")])

    m = accounts_module()
    accounts_action(m, "deposit")["effects"].append(remove("amount", loc(A, 43)))  # type: ignore[union-attr]
    cases["remove_non_state"] = (m, [err("TYPE_MISMATCH", A, 43)])

    m = accounts_module()
    accounts_action(m, "deposit")["effects"].append(remove("account", loc(A, 43)))  # type: ignore[union-attr]
    cases["remove_and_update"] = (m, [err("LIFECYCLE_CONFLICT", A, 43)])

    m = accounts_module()
    accounts_action(m, "create_twice")["effects"][1]["id"] = par("a", loc(A, 71))  # type: ignore[index]
    cases["create_same_input_twice"] = (m, [err("LIFECYCLE_CONFLICT", A, 71)])
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
    dump(valid / "fixed_scale.json", fixed_scale_module())
    dump(valid / "exact_bound_ok.json", bound_module(4))
    dump(valid / "exact_closure.json", exact_closure_module())
    dump(valid / "ledger.json", ledger_module())
    dump(valid / "accounts.json", accounts_module())
    for name, (doc, errors) in {**invalid_cases(), **constraint_invalid_cases(), **fixed_scale_invalid_cases(),
                           "exact_bound_exceeded": (bound_module(5), [err("EXACT_BOUND_EXCEEDED", B, 10)]),
                           **exact_closure_invalid_cases(), **decimal_store_invalid_cases(),
                           **lifecycle_invalid_cases()}.items():
        dump(invalid / f"{name}.json", doc)
        dump(invalid / f"{name}.expected.json", {"errors": errors})


if __name__ == "__main__":
    main()
