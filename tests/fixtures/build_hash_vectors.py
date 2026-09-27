"""Builds the inputs of tests/fixtures/hash_vectors.json (wire snippets without hashes).

Expected hashes are filled in once by `BLESS_HASH_VECTORS=1 cargo test --test hash_vectors`,
reviewed by hand against contracts/hashing.md, and then frozen. Changing a frozen hash
requires new hash tags (`…v2`).
"""

from __future__ import annotations

import json
import pathlib

OUT = pathlib.Path(__file__).resolve().parent / "hash_vectors.json"
L = {"file": "v.py", "line": 1}
I, D, S, B = {"t": "int"}, {"t": "decimal"}, {"t": "string"}, {"t": "bool"}
MONEY = {"t": "nominal", "name": "Money"}
ST = {"t": "enum", "name": "Status"}


def f(field: str) -> dict:
    return {"op": "field", "param": "e", "field": field, "loc": L}


def lit(ty: dict, v: object) -> dict:
    return {"op": "lit", "type": ty, "value": v, "loc": L}


def op(name: str, *args: dict, **extra: object) -> dict:
    return {"op": name, "args": list(args), "loc": L, **extra}


def base(**parts: list) -> dict:
    m = {
        "ir_version": "0.4", "constraints": [],
        "enums": [{"name": "Status", "values": ["a", "b"], "loc": L}],
        "nominals": [{"name": "Money", "underlying": D, "ops": ["add", "order", "ratio", "scale"], "loc": L}],
        "entities": [{"name": "E", "loc": L, "fields": [
            {"name": "i", "type": I, "loc": L}, {"name": "d", "type": D, "loc": L},
            {"name": "s", "type": S, "loc": L}, {"name": "b", "type": B, "loc": L},
            {"name": "m", "type": MONEY, "loc": L}, {"name": "st", "type": ST, "loc": L},
            {"name": "o", "type": {"t": "option", "of": {"t": "id", "entity": "E"}}, "loc": L}]}],
        "derived": [], "invariants": [], "actions": [],
    }
    m.update(parts)
    return m


def derived(body: dict, name: str = "v", kind: str = "derived") -> dict:
    return {"name": name, "kind": kind, "loc": L,
            "params": [{"name": "e", "type": {"t": "entity", "name": "E"}}], "body": body}


BODIES = {
    "lit_int": lit(I, -7), "lit_decimal": lit(D, "12.50"), "lit_string": lit(S, "x"),
    "lit_bool": lit(B, True), "lit_enum": lit(ST, "b"), "lit_nominal": lit(MONEY, "3"),
    "lit_option_none": lit({"t": "option", "of": {"t": "id", "entity": "E"}}, None),
    "field": f("i"),
    "eq": op("eq", f("s"), lit(S, "x")), "ne": op("ne", f("st"), lit(ST, "a")),
    "lt": op("lt", f("i"), lit(I, 3)), "le": op("le", f("d"), lit(D, "1")),
    "gt": op("gt", f("m"), lit(MONEY, "0")), "ge": op("ge", f("i"), f("i")),
    "add": op("add", f("i"), lit(I, 1)), "sub": op("sub", f("m"), f("m")),
    "mul": op("mul", f("m"), lit(I, 2)), "div": op("div", f("d"), f("i")),
    "and": op("and", f("b"), f("b"), f("b")), "or": op("or", f("b"), op("not", f("b"))),
    "not": op("not", f("b")), "in": {"op": "in", "args": [f("s")], "values": ["x", "y"], "loc": L},
    "is_none": op("is_none", f("o")), "is_some": op("is_some", f("o")),
    "value_or": op("value_or", f("o"), f("id")), "some": op("some", f("i")),
    "to_decimal": op("to_decimal", f("i")), "wrap": {"op": "wrap", "nominal": "Money", "args": [f("d")], "loc": L},
    "unwrap": op("unwrap", f("m")),
}


def main() -> None:
    vectors = []
    for name, body in BODIES.items():
        vectors.append({"name": f"expr_{name}", "wire": base(derived=[derived(body)])})
    vectors.append({"name": "expr_derived_ref", "wire": base(derived=[
        derived(op("add", f("i"), lit(I, 1)), "inner"),
        derived({"op": "derived", "name": "inner", "args": ["e"], "loc": L}, "outer")])})
    vectors.append({"name": "rule", "wire": base(derived=[derived(f("b"), "r", "rule")])})
    vectors.append({"name": "expr_param", "wire": base(actions=[{
        "name": "act", "loc": L,
        "params": [{"name": "e", "role": "state", "type": {"t": "entity", "name": "E"}},
                   {"name": "k", "role": "input", "type": I}],
        "preconditions": [{"expr": op("lt", {"op": "param", "param": "k", "loc": L}, f("i")), "loc": L}],
        "effects": [{"target": {"param": "e", "field": "i"}, "value": {"op": "param", "param": "k", "loc": L}, "loc": L}],
        "postconditions": [{"expr": op("eq", f("i"), {"op": "param", "param": "k", "loc": L}), "loc": L}]}])})
    vectors.append({"name": "invariant", "wire": base(invariants=[
        {"name": "inv", "entity": "E", "param": "e", "body": op("ge", f("i"), lit(I, 0)), "loc": L}])})
    vectors.append({"name": "decl_enum_only", "wire": {
        "ir_version": "0.4", "constraints": [], "enums": [{"name": "Status", "values": ["a", "b"], "loc": L}],
        "nominals": [], "entities": [], "derived": [], "invariants": [], "actions": []}})
    vectors.append({"name": "decl_nominal_only", "wire": {
        "ir_version": "0.4", "constraints": [], "enums": [], "entities": [], "derived": [], "invariants": [], "actions": [],
        "nominals": [{"name": "Money", "underlying": D, "ops": ["add", "order", "ratio", "scale"], "loc": L}]}})
    vectors.append({"name": "decl_entity_only", "wire": {
        "ir_version": "0.4", "constraints": [], "enums": [], "nominals": [], "derived": [], "invariants": [], "actions": [],
        "entities": [{"name": "U", "loc": L, "fields": [{"name": "name", "type": S, "loc": L}]}]}})
    vectors.append({"name": "empty_module", "wire": {
        "ir_version": "0.4", "constraints": [], "enums": [], "nominals": [], "entities": [], "derived": [], "invariants": [], "actions": []}})
    OUT.write_text(json.dumps({
        "_comment": "Frozen content-hash vectors (contracts/hashing.md). Any change requires new hash tags.",
        "vectors": vectors}, indent=1) + "\n")


if __name__ == "__main__":
    main()
