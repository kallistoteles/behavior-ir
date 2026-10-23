"""Installed artifact provenance parsing, not certificate authority."""
import json
from pathlib import Path
import sys
import pytest

sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
import cold_worker


def test_installed_wheel_digest_requires_actual_archive_identity():
    value={'archive_info':{'hashes':{'sha256':'a'*64}},'url':'file:///trusted.whl'}
    assert cold_worker.parse_installed_wheel_hash(json.dumps(value))=='a'*64
    for value in [None,'{}',json.dumps({'dir_info':{'editable':True}}),
                  json.dumps({'archive_info':{'hashes':{'sha256':True}}}),
                  json.dumps({'archive_info':{'hashes':{'sha256':'not-a-hash'}}})]:
        with pytest.raises(ValueError):cold_worker.parse_installed_wheel_hash(value)
