"""margin_model with `margin` renamed to `gross_margin`; bodies unchanged."""

from __future__ import annotations

from decimal import Decimal

from behavior import BehaviorModule, action, derived, entity, field, nominal, requires, rule, set_

Money = nominal("Money", Decimal, ops={"order", "add", "scale", "ratio"})


@entity
class Project:
    revenue = field(Money)
    cost = field(Money)
    flagged = field(bool)


@derived
def gross_margin(project: Project):
    return (project.revenue - project.cost) / project.revenue


@rule
def high_risk(project: Project):
    return gross_margin(project) < Decimal("0.05")


@action
def flag_project(project: Project):
    requires((project.revenue != Money(Decimal("0"))) & high_risk(project))
    set_(project.flagged, True)


@action
def flag_project_unguarded(project: Project):
    requires(high_risk(project))
    set_(project.flagged, True)


model = BehaviorModule(
    entities=[Project],
    derived=[gross_margin, high_risk],
    actions=[flag_project, flag_project_unguarded],
)
