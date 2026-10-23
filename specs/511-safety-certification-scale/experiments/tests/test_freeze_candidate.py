"""Candidate identity controls, never mathematical proof evidence."""
import sys
from pathlib import Path

import pytest

sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
import freeze_candidate


def test_input_hash_binds_types_names_and_all_declared_bytes(tmp_path):
    (tmp_path/'a.rs').write_text('original')
    first=freeze_candidate.hash_inputs(tmp_path,['a.rs'])
    assert first==freeze_candidate.hash_inputs(tmp_path,['a.rs'])
    (tmp_path/'a.rs').write_text('changed')
    assert first!=freeze_candidate.hash_inputs(tmp_path,['a.rs'])
    (tmp_path/'b.rs').write_text('original')
    assert first!=freeze_candidate.hash_inputs(tmp_path,['b.rs'])
    with pytest.raises(ValueError):freeze_candidate.hash_inputs(tmp_path,['../outside'])
    with pytest.raises(FileNotFoundError):freeze_candidate.hash_inputs(tmp_path,['missing.rs'])


def test_frozen_identity_file_cannot_be_overwritten(tmp_path):
    path=tmp_path/'manifest.json'
    freeze_candidate.write_frozen(path,{'state':'Frozen','identity':'first'})
    with pytest.raises(FileExistsError):freeze_candidate.write_frozen(path,{'state':'Frozen','identity':'changed'})
    assert 'first' in path.read_text()
