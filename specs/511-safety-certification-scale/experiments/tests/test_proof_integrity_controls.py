"""Mutation construction tests; actual Lean rejection is separate evidence."""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import proof_integrity_controls as controls


def test_statement_two_controls_change_actual_target_and_keep_parent():
    source = ('import safety.Check\nnamespace Concrete\n'
              'noncomputable def inventory := Safety.Candidate.compactPayload "payload"\n'
              'noncomputable def data : Safety.ModuleData := {\nbehaviorHash := [115]\n'
              'policies := [⟨0,[(1, [0])],[],[]⟩]\n}\n'
              'theorem authority : Safety.checkAuthority data = true := by decide\n'
              'theorem checked : Safety.checkModule data = true := Safety.Candidate.checked_from_parts data unique bounds authority link lifecycle\n'
              'theorem certified : Safety.InstanceGuarantee data b i p a c := witness\nend Concrete\n')
    variants = controls.mutations(source)
    assert set(variants) >= {'unauthorized_writer','proof_hole','weakened_statement','wrong_identity','wrong_payload','custom_axiom','native_axiom'}
    assert all(value != source for value in variants.values())
    assert 'by sorry' in variants['proof_hole']
    assert 'theorem certified : True' in variants['weakened_statement']
    assert 'compactPayload "xpayload"' in variants['wrong_payload']
    assert 'policies := [⟨0,[(1, [])],[],[]⟩]' in variants['unauthorized_writer']
    assert 'axiom UntrustedPremise' in variants['custom_axiom']
