"""Diagnostic transformations preserve full source data, never issue certificates."""
from pathlib import Path
import json
import sys

import pytest

EXPERIMENTS=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(EXPERIMENTS/'candidates'))
from candidate_encodings import compact_source


def original(inventory):
    leaf=','.join(str(b) for b in inventory.encode())
    return ('import safety.Check\nnamespace ConcreteAnchors\n'
        f'def inventory0 : List Nat := [{leaf}]\n'
        'def inventory : List (List Nat) := [inventory0]\n'
        'def policies0 : List Nat := [91,93]\n'
        'def policies : List (List Nat) := [policies0]\n'
        'end ConcreteAnchors\nnamespace Concrete\ndef data : Safety.ModuleData := {\n'
        'inventoryPayload := ConcreteAnchors.inventory\npolicyPayload := ConcreteAnchors.policies\n'
        'actionHashes := [[1,2,3],[4,5,6]]\nactions := []\n}\n'
        'theorem checked : Safety.checkModule data = true := by decide\nend Concrete\n')


@pytest.mark.parametrize('payload',['{"value":"ordinary"}','{"value":"åäö日本語"}',
    '{"value":"\\\\\\\"\\nnamespace Evil"}'])
def test_compact_source_preserves_utf8_and_exact_complete_keys(payload):
    source,details=compact_source(original(payload))
    assert details['inventory_utf8_hex']==payload.encode().hex()
    assert details['policies_utf8_hex']==b'[]'.hex()
    assert details['action_hashes']==[[1,2,3],[4,5,6]]
    assert 'def inventory0 : List Nat' not in source
    assert 'import CompactPayload' in source
    assert 'Safety.Candidate.compactPayload ' in source
    assert 'actionHashes := [candidateHash0,candidateHash1]' in source
    assert 'actions := []' in source
    encoded=next(l.split('compactPayload ',1)[1] for l in source.splitlines() if l.startswith('def inventory :'))
    assert json.loads(encoded)==payload


def test_no_partial_unparseable_source_is_silently_accepted():
    with pytest.raises(ValueError):compact_source('untrusted arbitrary text')


def test_raw_nat_key_literals_preserve_every_original_byte():
    source,details=compact_source(original('{"value":"x"}'),raw_literals=True)
    assert 'def candidateHash0 : List Nat := [(nat_lit 1),(nat_lit 2),(nat_lit 3)]' in source
    assert details['action_hashes']==[[1,2,3],[4,5,6]]


def test_proof_only_noncomputable_encoding_keeps_concrete_definition_bodies():
    ordinary,details=compact_source(original('{"value":"x"}'),raw_literals=True)
    proof_only,other=compact_source(original('{"value":"x"}'),raw_literals=True,proof_only=True)
    assert proof_only.replace('noncomputable def ','def ')==ordinary
    assert other==details
    assert 'noncomputable def data :' in proof_only
