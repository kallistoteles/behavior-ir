# Authoring evaluation: requirements

You are building a small order-management domain with Behavior. Write the model in `model.py`,
exporting `model` (a `BehaviorModule`). For each requirement below, either express it in the model
or, if the current Behavior release cannot express it, record a gap in `SEMANTIC_GAPS.md`. For
every requirement you express, add a check to `checks.py` that evaluates one allowed and one
denied case and asserts the results.

## Domain

Customers have a name, a region, and a credit limit in money with two decimals. Orders belong to a
customer and have an amount (money) and a status: open, closed or blocked. Employees have a unique
personnel number. Invoices have an amount, a status (pending or approved) and, once approved, the
approving user. Users have a role and an approval limit.

## Requirements

1. An order's amount is never negative.
2. A new order may be placed only if the customer's open and blocked orders plus the new amount
   stay within the customer's credit limit.
3. A customer can be closed (removed) only when they have no open orders.
4. No two employees ever share a personnel number.
5. Hiring an employee is refused if the personnel number is already taken.
6. An invoice can be approved only by a user whose role is "manager" and whose approval limit
   covers the amount; approval records the approver.
7. A pending invoice may get a discount; the amount stays non-negative.
8. Raising a customer's credit limit is allowed only upward.
9. Splitting a customer's credit into three equal parts stores each part rounded half-even to
   cents.
10. Closing an order sets its status to closed and is allowed only for open orders.
11. A customer may have at most three open orders at any time, whatever action opens them.
12. Only orders of customers in the "north" region may be blocked in bulk.
13. Closing a customer closes all of their open orders in the same transition.
14. The dashboard shows each customer's newest order.
