"""Public-package correspondence for the Core 029/030 adoption; no private Core imports."""
from __future__ import annotations

import importlib.util
from pathlib import Path
import sys

import behavior as b
import pytest

ROOT = Path(__file__).resolve().parents[2]
CASE_FILE = ROOT / "specs/512-adopt-core-update/probes/adoption_cases.py"
_spec = importlib.util.spec_from_file_location("core_update_adoption_cases", CASE_FILE)
assert _spec is not None and _spec.loader is not None
cases = importlib.util.module_from_spec(_spec)
sys.modules[_spec.name] = cases
_spec.loader.exec_module(cases)


@pytest.mark.parametrize("profile", ["0.7", "0.8"])
@pytest.mark.parametrize("pattern", ["free", "joint", "retarget"])
def test_supported_removal_has_real_proofs_and_allowed_replay(pattern: str, profile: str) -> None:
    row = cases.positive(pattern, profile)
    assert row["supported"], row["report"]
    assert row["runtime"]["allowed_position"] == 1
    assert row["runtime"]["occupied_refused_position"] == 0


@pytest.mark.parametrize("profile", ["0.7", "0.8"])
@pytest.mark.parametrize("action", [cases.unguarded, cases.create_dangling])
def test_unsafe_reference_changes_never_gain_proof_authority(action, profile: str) -> None:
    row = cases.negative(action, profile)
    assert row["position"] == 0 and row["runtime_result"] == "DENY"


@pytest.mark.parametrize("profile", ["0.7", "0.8"])
def test_other_incoming_classes_and_fields_stay_in_the_inventory(profile: str) -> None:
    report = cases.extra_incoming_class(profile)
    refs = [c for c in report.checks if c["subject"]["kind"] == "reference_world"]
    assert {c["subject"]["name"] for c in refs} == {
        "ref(Holder.target)", "ref(OtherHolder.target)", "ref(OtherHolder.second)",
    }
    assert not report.verified
    assert all(c["outcome"] != "proven" for c in refs if "OtherHolder" in c["subject"]["name"])


@pytest.mark.parametrize("profile", ["0.7", "0.8"])
def test_real_solver_exhaustion_is_inconclusive(profile: str) -> None:
    report = b.verify(cases.model_for(cases.remove_free, profile),
                      b.Profile(checks=["preservation"], rlimit=1))
    assert not report.verified and report.checks
    assert all(c["outcome"] == "inconclusive" for c in report.checks)
    assert any(c.get("reason") == "resource_limit" for c in report.checks)
    assert all("counterexample" not in finding for finding in report.findings)


@pytest.mark.parametrize("profile", ["0.7", "0.8"])
def test_unsupported_bound_order_is_not_a_counterexample(profile: str) -> None:
    report = cases.unsupported_bound_order(profile)
    assert not report.verified and report.checks
    assert all(c["outcome"] == "inconclusive" and c["reason"] == "encoding_error"
               for c in report.checks)
    assert all("counterexample" not in finding for finding in report.findings)


def test_cache_keeps_equal_body_action_scope_and_complete_incoming_schema(tmp_path: Path) -> None:
    @b.action
    def remove_alias(target: cases.Target):
        b.requires(b.not_(b.referenced(target.id)))
        b.remove(target)

    profile = b.Profile(checks=["preservation"])
    cache = str(tmp_path / "cache")
    module = b.BehaviorModule(entities=[cases.Target, cases.Holder],
                             actions=[cases.remove_free, remove_alias], reads=[cases.target_value])
    cold = b.verify(module, profile, cache=cache)
    assert cold.verified and cold.checks and all(not c["cached"] for c in cold.checks)
    assert len({c["action"]["hash"] for c in cold.checks}) == 1
    assert {c["action"]["name"] for c in cold.checks} == {"remove_free", "remove_alias"}
    warm = b.verify(module, profile, cache=cache)
    assert warm.hash == cold.hash and all(c["cached"] for c in warm.checks)
    changed = b.BehaviorModule(entities=[cases.Target, cases.Holder, cases.OtherHolder],
                              actions=[cases.remove_free, remove_alias], reads=[cases.target_value])
    changed_cold = b.verify(changed, profile, cache=cache)
    assert changed_cold.verified and all(not c["cached"] for c in changed_cold.checks)
    for name in ["remove_free", "remove_alias"]:
        assert len([c for c in changed_cold.checks if c["subject"]["kind"] == "reference_world"
                    and c["action"]["name"] == name]) == 3
    assert b.verify(changed, profile).hash == changed_cold.hash
    assert all(c["cached"] for c in b.verify(changed, profile, cache=cache).checks)
    different_profile = b.Profile(checks=["preservation"], rlimit=profile.rlimit - 1)
    assert all(not c["cached"] for c in b.verify(changed, different_profile, cache=cache).checks)


def test_previous_verifier_cache_cannot_replace_a_fresh_current_check(tmp_path: Path) -> None:
    import json
    archive = json.loads((ROOT / "specs/512-adopt-core-update/evidence/baseline-verifier-cache.json").read_text())
    assert archive["versions"]["verifier"] == "0.10.0"
    assert archive["files"]
    cache = tmp_path / "cache"
    cache.mkdir()
    for relative, content in archive["files"].items():
        path = cache / relative
        assert path.resolve().is_relative_to(cache.resolve())
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content)
    model = cases.cache_model()
    assert model.behavior_version == archive["behavior_hash"]
    profile = b.Profile(checks=["postcondition"])
    fresh = b.verify(model, profile, cache=str(cache))
    assert fresh.verified and fresh.checks and all(not c["cached"] for c in fresh.checks)
    assert fresh.data["verifier_version"] == "0.11.0"
    assert fresh.hash != archive["report"]["hash"]
    warm = b.verify(model, profile, cache=str(cache))
    assert warm.hash == fresh.hash and all(c["cached"] for c in warm.checks)
