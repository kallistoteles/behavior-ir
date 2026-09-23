"""Derived values that form a cycle: a → b → c → a."""

from __future__ import annotations

from behavior import BehaviorModule, derived, entity, field


@entity
class Node:
    weight = field(int)


@derived
def a(n: Node):
    return b(n) + 1


@derived
def b(n: Node):
    return c(n) + 1


@derived
def c(n: Node):
    return a(n) + n.weight


model = BehaviorModule(entities=[Node], derived=[a, b, c])
