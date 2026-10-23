"""Installed-smoke process boundary tests; mocks are not proof evidence."""
from pathlib import Path
import importlib.util
import json
import subprocess

import pytest

ROOT=Path(__file__).resolve().parents[4]
spec=importlib.util.spec_from_file_location('release_safety_smoke',ROOT/'release/safety_smoke.py')
smoke=importlib.util.module_from_spec(spec);spec.loader.exec_module(smoke)


def test_verification_uses_separate_process_and_denies_producer_files(tmp_path,monkeypatch):
    producer=tmp_path/'producer';producer.mkdir();(producer/'secret').write_text('producer only')
    bundle={'unverified':'data only'}
    def run(command,**kwargs):
        assert '--verify-bundle' in command
        copied=Path(command[command.index('--verify-bundle')+1])
        assert copied.parent != producer
        assert json.loads(copied.read_text())==bundle
        with pytest.raises(PermissionError):(producer/'secret').read_text()
        return subprocess.CompletedProcess(command,0,'{"evidence_hash":"same"}\n','')
    monkeypatch.setattr(smoke.subprocess,'run',run)
    assert smoke.verify_in_consumer(bundle,producer,'pinned')['evidence_hash']=='same'
    assert (producer/'secret').read_text()=='producer only'


def test_failed_consumer_restores_access_and_propagates_failure(tmp_path,monkeypatch):
    producer=tmp_path/'producer';producer.mkdir()
    def run(*args,**kwargs):raise subprocess.CalledProcessError(7,args[0])
    monkeypatch.setattr(smoke.subprocess,'run',run)
    with pytest.raises(subprocess.CalledProcessError):smoke.verify_in_consumer({},producer,'pinned')
    assert list(producer.iterdir())==[]
