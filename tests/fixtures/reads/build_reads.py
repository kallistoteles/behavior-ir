"""Builds the read fixtures in tests/fixtures/reads (feature 010).

Like the wire and migration fixtures, everything is written here as Python data, independent of
the DSL under test. Run from the repository root:

    python tests/fixtures/reads/build_reads.py

Layout:
- `modules/lab.json`: a wire IR 0.7 module with declared reads (the lab domain);
- `modules/lab_base.json`: the same module without reads, at wire IR 0.6;
- `modules/lab_empty_reads.json`: the same module at 0.7 with an empty `reads` section;
- `valid/<case>.json`: ad-hoc read documents that must be admitted against `lab`;
- `invalid/<case>.json` + `<case>.expected.json`: modules or read documents that must be
  refused, with the expected error codes;
- `../read_intents/<case>.json`: read intents (capability calls) against `lab`, with `host.json`;
  rejected ones have `<case>.expected.json` with their problems;
- `requests/<case>.json`: plain read requests against `lab` (the `behavior read` format), whose
  golden records `records/<case>.expected.json` are blessed by
  `BLESS_READ_GOLDENS=1 cargo test -p behavior-core --test read_goldens` and reviewed.

Outputs are canonical JSON (sorted keys, compact) and are committed. Review diffs by hand.
"""

from __future__ import annotations

import copy
import json
import pathlib
from typing import Any

ROOT = pathlib.Path(__file__).resolve().parent
J = dict[str, Any]
F = "lab.py"


def dump(path: pathlib.Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False),
        encoding="utf-8",
    )


def loc(line: int, file: str = F) -> J:
    return {"file": file, "line": line}


# --- wire builders ----------------------------------------------------------------------------

BOOL = {"t": "bool"}
INT = {"t": "int"}
STR = {"t": "string"}


def enum(name: str) -> J:
    return {"t": "enum", "name": name}


def opt(of: J) -> J:
    return {"t": "option", "of": of}


def ent(name: str) -> J:
    return {"t": "entity", "name": name}


def ref(entity: str) -> J:
    return {"t": "ref", "entity": entity}


def id_of(entity: str) -> J:
    return {"t": "id", "entity": entity}


def field(name: str, ty: J, line: int) -> J:
    return {"name": name, "type": ty, "loc": loc(line)}


def lit(ty: J, value: object, line: int) -> J:
    return {"op": "lit", "type": ty, "value": value, "loc": loc(line)}


def fld(param: str, name: str, line: int) -> J:
    return {"op": "field", "param": param, "field": name, "loc": loc(line)}


def par(name: str, line: int) -> J:
    return {"op": "param", "param": name, "loc": loc(line)}


def op(name: str, *args: J, line: int) -> J:
    return {"op": name, "args": list(args), "loc": loc(line)}


def call(name: str, *args: str, line: int) -> J:
    return {"op": "derived", "name": name, "args": list(args), "loc": loc(line)}


def select(entity: str, line: int) -> J:
    return {"op": "select", "entity": entity, "loc": loc(line)}


def lam(name: str, query: J, param: str, body: J, line: int) -> J:
    return {"op": name, "args": [query], "param": param, "body": body, "loc": loc(line)}


def param(name: str, role: str, ty: J) -> J:
    return {"name": name, "role": role, "type": ty}


def value_read(name: str, params: list[J], body: J, line: int) -> J:
    return {"name": name, "params": params, "body": {"value": body}, "loc": loc(line)}


def project_read(name: str, params: list[J], over: J, member: str, items: list[J],
                 line: int) -> J:
    return {"name": name, "params": params, "loc": loc(line),
            "body": {"project": {"over": over, "param": member, "items": items}}}


def f_item(name: str) -> J:
    return {"field": name}


def d_item(name: str) -> J:
    return {"derived": name}


def active_cultures_query(line: int) -> J:
    return lam("where", select("Culture", line), "c", fld("c", "active", line), line)


# --- the lab domain ---------------------------------------------------------------------------

def base() -> J:
    """The lab domain without reads (wire IR 0.6)."""
    return {
        "ir_version": "0.6",
        "enums": [
            {"name": "Stage", "values": ["SEED", "GROWTH", "HARVEST"], "loc": loc(3)},
            {"name": "OrderStatus", "values": ["OPEN", "CLOSED"], "loc": loc(4)},
        ],
        "nominals": [],
        "entities": [
            {"name": "Culture", "loc": loc(7), "fields": [
                field("name", STR, 8),
                field("stage", opt(enum("Stage")), 9),
                field("active", BOOL, 10),
                field("measurements", INT, 11),
                field("ph_total", INT, 12),
            ]},
            {"name": "Customer", "loc": loc(14), "fields": [
                field("name", STR, 15),
                field("credit_limit", INT, 16),
            ]},
            {"name": "Order", "loc": loc(18), "fields": [
                field("customer", ref("Customer"), 19),
                field("amount", INT, 20),
                field("status", enum("OrderStatus"), 21),
            ]},
        ],
        "derived": [
            # The average pH reading of a culture: fails for a culture without measurements.
            {"name": "ph_avg", "kind": "derived", "loc": loc(24),
             "params": [{"name": "c", "type": ent("Culture")}],
             "body": op("div", fld("c", "ph_total", 25), fld("c", "measurements", 25), line=25)},
            # The total of a customer's open orders.
            {"name": "exposure", "kind": "derived", "loc": loc(27),
             "params": [{"name": "c", "type": ent("Customer")}],
             "body": lam("sum", lam("where", select("Order", 28), "o", op(
                 "and",
                 op("eq", fld("o", "customer", 28), fld("c", "id", 28), line=28),
                 op("eq", fld("o", "status", 28), lit(enum("OrderStatus"), "OPEN", 28), line=28),
                 line=28), 28), "o", fld("o", "amount", 28), 28)},
            # Whether the open orders are within the credit limit (reads the internal limit).
            {"name": "standing", "kind": "derived", "loc": loc(30),
             "params": [{"name": "c", "type": ent("Customer")}],
             "body": op("le", call("exposure", "c", line=31), fld("c", "credit_limit", 31),
                        line=31)},
        ],
        "invariants": [],
        "constraints": [
            {"name": "measurements_not_negative", "entity": "Culture", "param": "c",
             "loc": loc(34),
             "body": op("ge", fld("c", "measurements", 35), lit(INT, 0, 35), line=35)},
        ],
        "actions": [
            {"name": "start_culture", "loc": loc(38),
             "params": [param("culture_id", "input", id_of("Culture")),
                        param("name", "input", STR)],
             "preconditions": [], "postconditions": [],
             "effects": [{"create": "Culture", "loc": loc(39),
                          "id": par("culture_id", 39),
                          "fields": {
                              "name": par("name", 39),
                              "stage": lit(opt(enum("Stage")), None, 39),
                              "active": lit(BOOL, True, 39),
                              "measurements": lit(INT, 0, 39),
                              "ph_total": lit(INT, 0, 39)}}]},
            {"name": "measure", "loc": loc(41),
             "params": [param("culture", "state", ent("Culture")), param("ph", "input", INT)],
             "preconditions": [], "postconditions": [],
             "effects": [
                 {"target": {"param": "culture", "field": "measurements"}, "loc": loc(42),
                  "value": op("add", fld("culture", "measurements", 42), lit(INT, 1, 42),
                              line=42)},
                 {"target": {"param": "culture", "field": "ph_total"}, "loc": loc(43),
                  "value": op("add", fld("culture", "ph_total", 43), par("ph", 43), line=43)},
             ]},
            {"name": "advance", "loc": loc(45),
             "params": [param("culture", "state", ent("Culture")),
                        param("stage", "input", enum("Stage"))],
             "preconditions": [], "postconditions": [],
             "effects": [{"target": {"param": "culture", "field": "stage"}, "loc": loc(46),
                          "value": op("some", par("stage", 46), line=46)}]},
            {"name": "retire", "loc": loc(48),
             "params": [param("culture", "state", ent("Culture"))],
             "preconditions": [], "postconditions": [],
             "effects": [{"target": {"param": "culture", "field": "active"}, "loc": loc(49),
                          "value": lit(BOOL, False, 49)}]},
            {"name": "register", "loc": loc(51),
             "params": [param("customer_id", "input", id_of("Customer")),
                        param("name", "input", STR), param("limit", "input", INT)],
             "preconditions": [], "postconditions": [],
             "effects": [{"create": "Customer", "loc": loc(52),
                          "id": par("customer_id", 52),
                          "fields": {"name": par("name", 52),
                                     "credit_limit": par("limit", 52)}}]},
            {"name": "place_order", "loc": loc(54),
             "params": [param("customer", "state", ent("Customer")),
                        param("order_id", "input", id_of("Order")),
                        param("amount", "input", INT)],
             "preconditions": [], "postconditions": [],
             "effects": [{"create": "Order", "loc": loc(55),
                          "id": par("order_id", 55),
                          "fields": {"customer": fld("customer", "id", 55),
                                     "amount": par("amount", 55),
                                     "status": lit(enum("OrderStatus"), "OPEN", 55)}}]},
            {"name": "close_order", "loc": loc(57),
             "params": [param("order", "state", ent("Order"))],
             "preconditions": [], "postconditions": [],
             "effects": [{"target": {"param": "order", "field": "status"}, "loc": loc(58),
                          "value": lit(enum("OrderStatus"), "CLOSED", 58)}]},
        ],
    }


def reads() -> list[J]:
    """The lab's declared reads: the questions its users and agents ask."""
    return [
        value_read("active_count", [],
                   op("count", active_cultures_query(61), line=61), 60),
        value_read("open_total", [param("customer", "state", ent("Customer"))],
                   call("exposure", "customer", line=64), 63),
        project_read("order_view", [param("order", "state", ent("Order"))],
                     par("order", 67), "o", [f_item("status"), f_item("amount")], 66),
        project_read("active_cultures", [], active_cultures_query(70), "c",
                     [f_item("name"), f_item("stage"), d_item("ph_avg")], 69),
        project_read("customer_summary", [param("customer", "state", ent("Customer"))],
                     par("customer", 73), "c", [f_item("name"), d_item("standing")], 72),
        value_read("smallest_order", [],
                   lam("min", select("Order", 76), "o", fld("o", "amount", 76), 76), 75),
        value_read("average_ph", [],
                   op("div",
                      lam("sum", select("Culture", 79), "c", fld("c", "ph_total", 79), 79),
                      op("count", select("Culture", 79), line=79), line=79), 78),
        project_read("big_orders", [param("threshold", "input", INT)],
                     lam("where", select("Order", 82), "o",
                         op("ge", fld("o", "amount", 82), par("threshold", 82), line=82), 82),
                     "o", [f_item("amount"), f_item("status"), f_item("customer")], 81),
    ]


def lab() -> J:
    w = base()
    w["ir_version"] = "0.7"
    w["reads"] = reads()
    return w


def read_doc(read: J) -> J:
    return {"ir_version": "0.7", "read": read}


def valid() -> dict[str, J]:
    """Ad-hoc read documents admitted against `lab`."""
    return {
        "customer_count": read_doc(value_read(
            "customer_count", [], op("count", select("Customer", 90), line=90), 90)),
        "culture_ph": read_doc(value_read(
            "culture_ph", [param("culture", "state", ent("Culture"))],
            call("ph_avg", "culture", line=92), 92)),
        "orders_over": read_doc(project_read(
            "orders_over", [param("min", "input", INT)],
            lam("where", select("Order", 94), "o",
                op("gt", fld("o", "amount", 94), par("min", 94), line=94), 94),
            "o", [f_item("amount")], 94)),
    }


def invalid() -> dict[str, tuple[J, list[str]]]:
    """Read documents (against `lab`) that must be refused, with the expected error codes."""
    return {
        "entity_typed_body": (read_doc(value_read(
            "whole", [param("culture", "state", ent("Culture"))], par("culture", 100), 100)),
            ["TYPE_MISMATCH"]),
        "query_typed_body": (read_doc(value_read(
            "everything", [], select("Culture", 101), 101)), ["TYPE_MISMATCH"]),
        "unknown_param_type": (read_doc(value_read(
            "nothing", [param("x", "input", {"t": "enum", "name": "Nope"})],
            lit(INT, 1, 102), 102)), ["UNKNOWN_TYPE"]),
        # Projections (US2).
        "unknown_field": (read_doc(project_read(
            "colours", [], select("Culture", 110), "c", [f_item("colour")], 110)),
            ["UNKNOWN_PROJECTION_ITEM"]),
        "reference_path": (read_doc(project_read(
            "buyers", [], select("Order", 111), "o", [f_item("customer.name")], 111)),
            ["UNKNOWN_PROJECTION_ITEM"]),
        "derived_of_another_type": (read_doc(project_read(
            "exposures", [], select("Culture", 112), "c", [d_item("exposure")], 112)),
            ["UNKNOWN_PROJECTION_ITEM"]),
        "unknown_derived": (read_doc(project_read(
            "nothings", [], select("Culture", 113), "c", [d_item("nothing")], 113)),
            ["UNKNOWN_PROJECTION_ITEM"]),
        "repeated_field": (read_doc(project_read(
            "twice", [], select("Culture", 114), "c", [f_item("name"), f_item("name")], 114)),
            ["DUPLICATE_PROJECTION_ITEM"]),
        "listed_id": (read_doc(project_read(
            "ids", [], select("Culture", 115), "c", [f_item("id")], 115)),
            ["DUPLICATE_PROJECTION_ITEM"]),
        "over_an_input": (read_doc(project_read(
            "inputs", [param("n", "input", INT)], par("n", 116), "x", [f_item("name")], 116)),
            ["INVALID_PROJECTION"]),
        "over_a_value": (read_doc(project_read(
            "values", [param("culture", "state", ent("Culture"))],
            fld("culture", "name", 117), "x", [f_item("name")], 117)),
            ["INVALID_PROJECTION"]),
        "non_local_filter": (read_doc(project_read(
            "linked", [],
            lam("where", select("Order", 118), "o",
                op("exists", fld("o", "customer", 118), line=118), 118),
            "o", [f_item("amount")], 118)),
            ["NON_LOCAL_PREDICATE"]),
        # A module whose read projects a derived value with two parameters, and one whose read
        # lists a field and a derived value of the same name.
        "two_parameter_derived": (two_parameter_derived(), ["UNKNOWN_PROJECTION_ITEM"]),
        "field_and_derived_same_name": (same_name(), ["DUPLICATE_PROJECTION_ITEM"]),
        "derived_named_id": (derived_named_id(), ["DUPLICATE_PROJECTION_ITEM"]),
        # Declared reads are entry points (US5): no module expression calls one, and a read
        # shares no name with an action or a derived value.
        **{f"read_call_in_{site}": (read_called_from(site), ["READ_CALL_NOT_ALLOWED"])
           for site in ("precondition", "effect", "rule", "invariant", "derived", "read")},
        "read_named_like_an_action": (renamed_read("measure"), ["DUPLICATE_CAPABILITY"]),
        "read_named_like_a_derived_value": (renamed_read("ph_avg"), ["DUPLICATE_CAPABILITY"]),
    }


GUIDANCE = "move the shared computation into a derived value"


def read_called_from(site: str) -> J:
    """`lab` with a module expression at `site` that calls the declared read `active_count`."""
    w = lab()
    call_read = call("active_count", line=130)
    non_negative = op("ge", call_read, lit(INT, 0, 130), line=130)
    if site == "precondition":
        w["actions"][1]["preconditions"] = [{"expr": non_negative, "loc": loc(130)}]
    elif site == "effect":
        w["actions"][1]["effects"][0]["value"] = call_read
    elif site == "rule":
        w["derived"].append({"name": "busy", "kind": "rule", "loc": loc(130),
                             "params": [{"name": "c", "type": ent("Culture")}],
                             "body": non_negative})
    elif site == "invariant":
        w["invariants"].append({"name": "counted", "entity": "Culture", "param": "c",
                                "loc": loc(130), "body": non_negative})
    elif site == "derived":
        w["derived"].append({"name": "how_many", "kind": "derived", "loc": loc(130),
                             "params": [{"name": "c", "type": ent("Culture")}],
                             "body": call_read})
    else:
        w["reads"].append(value_read("twice_as_many", [],
                                     op("mul", call_read, lit(INT, 2, 130), line=130), 130))
    return w


def renamed_read(name: str) -> J:
    w = lab()
    w["reads"][0] = dict(w["reads"][0], name=name)
    return w


def intents() -> dict[str, tuple[J, list[tuple[str, str]] | None]]:
    """Read intents against `lab` with `read_intents/host.json`: rejected ones with their
    (code, path) problems in path order, accepted ones with None."""
    return {
        "unknown_read": ({"capability": "nope"}, [("UNKNOWN_CAPABILITY", "capability")]),
        "action_name": ({"capability": "measure", "targets": {"culture": "c1"},
                         "input": {"ph": 7}}, [("UNKNOWN_CAPABILITY", "capability")]),
        "missing_target": ({"capability": "open_total"},
                           [("MISSING_TARGET", "targets.customer")]),
        "extra_target": ({"capability": "active_count", "targets": {"culture": "c1"}},
                         [("EXTRA_TARGET", "targets.culture")]),
        "missing_input": ({"capability": "big_orders"},
                          [("MISSING_ARGUMENT", "input.threshold")]),
        "extra_input": ({"capability": "active_count", "input": {"x": 1}},
                        [("EXTRA_ARGUMENT", "input.x")]),
        "wrong_type": ({"capability": "big_orders", "input": {"threshold": "many"}},
                       [("WRONG_TYPE", "input.threshold")]),
        "smuggled_definition": ({"capability": "active_count",
                                 "definition": valid()["customer_count"]},
                                [("EXTRA_ARGUMENT", "definition")]),
        "smuggled_context": ({"capability": "active_count", "context": {"actor": "me"}},
                             [("EXTRA_ARGUMENT", "context")]),
        "target_mismatch": ({"capability": "open_total", "targets": {"customer": "k2"}},
                            [("TARGET_MISMATCH", "targets.customer")]),
        "multiple_problems": ({"capability": "big_orders", "targets": {"x": "1"},
                               "input": {"threshold": "a", "y": 2}},
                              [("WRONG_TYPE", "input.threshold"), ("EXTRA_ARGUMENT", "input.y"),
                               ("EXTRA_TARGET", "targets.x")]),
        "valid_value": ({"capability": "active_count"}, None),
        "valid_projection": ({"capability": "customer_summary",
                              "targets": {"customer": "k1"}}, None),
    }


def intent_host() -> J:
    """The trusted host's side of a read intent in plain mode: state, context, data version
    and facts."""
    return {
        "data_version": "intent:1",
        "state": {"customer": {"id": "k1", "name": "Ada", "credit_limit": 100}},
        "context": {},
        "facts": {"universe": [
            {"entity": "Culture", "members": [
                {"id": "c1", "name": "Basil", "stage": None, "active": True, "measurements": 1,
                 "ph_total": 7}]},
            {"entity": "Order", "members": [
                {"id": "o1", "customer": "k1", "amount": 30, "status": "OPEN"}]}]},
    }


def two_parameter_derived() -> J:
    w = lab()
    w["derived"].append(
        {"name": "ph_gap", "kind": "derived", "loc": loc(120),
         "params": [{"name": "a", "type": ent("Culture")}, {"name": "b", "type": ent("Culture")}],
         "body": op("sub", fld("a", "ph_total", 120), fld("b", "ph_total", 120), line=120)})
    w["reads"] = [project_read("gaps", [], select("Culture", 121), "c", [d_item("ph_gap")], 121)]
    return w


def derived_named_id() -> J:
    """A derived value named `id`: projecting it would overwrite every record's identity."""
    w = lab()
    w["derived"].append(
        {"name": "id", "kind": "derived", "loc": loc(124),
         "params": [{"name": "c", "type": ent("Culture")}], "body": fld("c", "name", 124)})
    w["reads"] = [project_read("ids", [], select("Culture", 125), "c", [d_item("id")], 125)]
    return w


def same_name() -> J:
    w = lab()
    w["derived"].append(
        {"name": "name", "kind": "derived", "loc": loc(122),
         "params": [{"name": "c", "type": ent("Culture")}], "body": fld("c", "name", 122)})
    w["reads"] = [project_read("names", [], select("Culture", 123), "c",
                               [f_item("name"), d_item("name")], 123)]
    return w


def requests() -> dict[str, J]:
    """Plain read requests against `lab` (the `behavior read` request format), whose records are
    the golden records in `records/`."""
    def culture(id_: str, active: bool, measurements: int, ph_total: int,
                stage: str | None = None) -> J:
        return {"id": id_, "name": f"culture {id_}", "stage": stage, "active": active,
                "measurements": measurements, "ph_total": ph_total}

    cultures = {"universe": [{"entity": "Culture", "members": [
        culture("c1", True, 2, 14, "GROWTH"), culture("c2", False, 0, 0),
        culture("c3", True, 1, 6)]}]}
    orders = {"universe": [{"entity": "Order", "members": [
        {"id": "o1", "customer": "k1", "amount": 30, "status": "OPEN"},
        {"id": "o2", "customer": "k1", "amount": 12, "status": "CLOSED"},
        {"id": "o3", "customer": "k2", "amount": 7, "status": "OPEN"}]}]}
    ada = {"customer": {"id": "k1", "name": "Ada", "credit_limit": 100}}

    def req(read: object, state: J | None = None, input_: J | None = None,
            facts: J | None = None) -> J:
        r: J = {"read": read, "data_version": "golden:1", "state": state or {},
                "input": input_ or {}, "context": {}}
        if facts is not None:
            r["facts"] = facts
        return r

    return {
        "value": req("active_count", facts=cultures),
        "bound_value": req("open_total", ada, facts=orders),
        "query_projection": req("active_cultures", facts=cultures),
        "entity_projection": req("customer_summary", ada, facts=orders),
        "evaluation_error": req("average_ph",
                                facts={"universe": [{"entity": "Culture", "members": []}]}),
        "invalid_input": req("open_total", input_={"unexpected": 1}),
        "invalid_binding": req("open_total", {"customer": {"id": "nobody"}},
                               facts={"existence": [
                                   {"entity": "Customer", "id": "nobody", "exists": False}]}),
        "ad_hoc": req({"definition": valid()["orders_over"]}, input_={"min": 10}, facts=orders),
    }


def verify_module() -> J:
    """A module whose declared reads can or cannot fail (feature 010, FR-017):
    `tests/fixtures/verify/reads.json`."""
    f = "readsv.py"
    def l(n: int) -> J:
        return loc(n, f)
    c = lambda name, n: {"op": "field", "param": "c", "field": name, "loc": l(n)}  # noqa: E731
    sel = {"op": "select", "entity": "Culture", "loc": l(20)}
    ph_sum = {"op": "sum", "args": [sel], "param": "c", "body": c("ph_total", 20), "loc": l(20)}
    n = {"op": "count", "args": [sel], "loc": l(20)}
    return {
        "ir_version": "0.7", "enums": [], "nominals": [], "invariants": [], "actions": [],
        "entities": [{"name": "Culture", "loc": l(3), "fields": [
            {"name": "measurements", "type": INT, "loc": l(4)},
            {"name": "ph_total", "type": INT, "loc": l(5)}]}],
        "constraints": [
            {"name": "counted", "entity": "Culture", "param": "c", "loc": l(7),
             "body": {"op": "ge", "args": [c("measurements", 7), lit(INT, 0, 7)], "loc": l(7)}},
            {"name": "bounded", "entity": "Culture", "param": "c", "loc": l(8),
             "body": {"op": "le", "args": [c("measurements", 8), lit(INT, 1000000, 8)],
                      "loc": l(8)}}],
        "derived": [{"name": "avg", "kind": "derived", "loc": l(10),
                     "params": [{"name": "c", "type": ent("Culture")}],
                     "body": {"op": "div", "args": [c("ph_total", 11), c("measurements", 11)],
                              "loc": l(11)}}],
        "reads": [
            # Divides by the number of cultures: fails on an empty state.
            {"name": "average_ph", "params": [], "loc": l(20),
             "body": {"value": {"op": "div", "args": [ph_sum, n], "loc": l(20)}}},
            # Divides by one more than a bounded, non-negative count: never zero, never overflows.
            {"name": "smoothed_ph", "loc": l(22),
             "params": [{"name": "culture", "role": "state", "type": ent("Culture")}],
             "body": {"value": {"op": "div", "loc": l(22), "args": [
                 {"op": "field", "param": "culture", "field": "ph_total", "loc": l(22)},
                 {"op": "add", "loc": l(22), "args": [
                     {"op": "field", "param": "culture", "field": "measurements", "loc": l(22)},
                     lit(INT, 1, 22)]}]}}},
            # Projects the average of every culture: fails for one without measurements.
            {"name": "all_averages", "params": [], "loc": l(24),
             "body": {"project": {"over": sel, "param": "c", "items": [{"derived": "avg"}]}}},
            # Only cultures with measurements: the filter guarantees a non-zero divisor.
            {"name": "measured_averages", "params": [], "loc": l(26),
             "body": {"project": {"param": "c", "items": [{"derived": "avg"}], "over": {
                 "op": "where", "args": [sel], "param": "c", "loc": l(26),
                 "body": {"op": "gt", "args": [c("measurements", 26), lit(INT, 0, 26)],
                          "loc": l(26)}}}}},
            # One bound culture's average: fails for one without measurements.
            {"name": "culture_view", "loc": l(28),
             "params": [{"name": "culture", "role": "state", "type": ent("Culture")}],
             "body": {"project": {"over": {"op": "param", "param": "culture", "loc": l(28)},
                                  "param": "c",
                                  "items": [{"field": "ph_total"}, {"derived": "avg"}]}}},
        ],
    }


def verify_expected() -> J:
    div = "(division by zero)"
    total = "decimal(sum(select(Culture), c, c.ph_total))"
    check = lambda read, subject, outcome: {  # noqa: E731
        "kind": "evaluation_error", "action": f"read:{read}", "subject": subject,
        "outcome": outcome}
    avg = "decimal(c.ph_total) / decimal(c.measurements)"
    return {
        "_comment": "Feature 010 (FR-017): evaluation errors of declared reads. A projection's "
                    "derived item is checked for a member that satisfies the query's filter. "
                    "average_ph also overflows: a sum of Int fields can exceed the Int range "
                    "(a real finding, confirmed by evaluating the read).",
        "profile": ["evaluation_error"],
        "checks": [
            check("average_ph", f"{total} / decimal(count(select(Culture))) {div}",
                  "counterexample"),
            check("average_ph", "$c.ph_total (numeric overflow)", "counterexample"),
            check("smoothed_ph",
                  f"decimal(culture.ph_total) / decimal(culture.measurements + 1) {div}",
                  "proven"),
            check("smoothed_ph", "culture.measurements + 1 (numeric overflow)", "proven"),
            check("all_averages", f"{avg} {div}", "counterexample"),
            check("measured_averages", f"{avg} {div}", "proven"),
            check("culture_view", f"{avg} {div}", "counterexample"),
        ],
    }


def main() -> None:
    dump(ROOT / "modules" / "lab.json", lab())
    dump(ROOT / "modules" / "lab_base.json", base())
    empty = copy.deepcopy(base())
    empty["ir_version"] = "0.7"
    empty["reads"] = []
    dump(ROOT / "modules" / "lab_empty_reads.json", empty)
    for name, doc in valid().items():
        dump(ROOT / "valid" / f"{name}.json", doc)
    for name, r in requests().items():
        dump(ROOT / "requests" / f"{name}.json", r)
    for name, (doc, codes) in invalid().items():
        dump(ROOT / "invalid" / f"{name}.json", doc)
        expected: J = {"codes": codes}
        if codes == ["READ_CALL_NOT_ALLOWED"]:
            expected["message"] = GUIDANCE
        dump(ROOT / "invalid" / f"{name}.expected.json", expected)
    verify_dir = ROOT.parent / "verify"
    (verify_dir / "reads.json").write_text(json.dumps(verify_module(), indent=1) + "\n")
    (verify_dir / "reads.expected.json").write_text(
        json.dumps(verify_expected(), indent=1) + "\n")
    intents_dir = ROOT.parent / "read_intents"
    dump(intents_dir / "host.json", intent_host())
    for name, (intent, problems) in intents().items():
        dump(intents_dir / f"{name}.json", intent)
        if problems is not None:
            dump(intents_dir / f"{name}.expected.json", {"rejected": True, "errors": [
                {"code": c, "path": p} for c, p in problems]})


if __name__ == "__main__":
    main()
