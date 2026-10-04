"""Verifying declared reads: `evaluation_error` covers every declared read. A projection's derived
values are checked for a member that passes the query's filter, so a filter that excludes the
failing case proves the read."""

from behavior import BehaviorModule, Profile, count, derived, entity, field, project, read, select
from behavior import sum_, verify


@entity
class Culture:
    measurements = field(int)
    ph_total = field(int)


@derived
def ph_avg(c: Culture):
    return c.ph_total / c.measurements


@read
def all_averages():
    return project(select(Culture), lambda c: [ph_avg(c)])


@read
def measured_averages():
    return project(select(Culture).where(lambda c: c.measurements > 0), lambda c: [ph_avg(c)])


@read
def average_ph():
    return sum_(select(Culture), lambda c: c.ph_total) / count(select(Culture))


model = BehaviorModule(entities=[Culture], derived=[ph_avg],
                       reads=[all_averages, measured_averages, average_ph])
attestation = verify(model, Profile(checks=["evaluation_error"]))
division = {c["action"]["name"]: c["outcome"] for c in attestation.checks
            if "division by zero" in c["subject"]["name"]}
assert division == {"read:all_averages": "counterexample",
                    "read:measured_averages": "proven",
                    "read:average_ph": "counterexample"}

# A counterexample is a real read of a real state: its record is in the finding.
finding = next(f for f in attestation.findings
               if f["explanation"].startswith("read:all_averages"))
assert finding["counterexample"]["record"]["result"] == "EVALUATION_ERROR"
assert not attestation.verified
print("verifying_reads: OK")
