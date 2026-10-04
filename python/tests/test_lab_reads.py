"""SC-001 (feature 010): the lab answers its questions with reads, without effect-free actions or
a placeholder entity."""

from __future__ import annotations

import io
import json
from contextlib import redirect_stdout

from examples.lab_reads import run
from examples.lab_reads.model import model


def test_every_action_changes_something() -> None:
    wire = json.loads(model.to_wire_json())
    for a in wire["actions"]:
        assert a["effects"], f"{a['name']} has no effect: questions are reads"
    assert len(wire["reads"]) >= 6


def test_the_example_runs_and_answers() -> None:
    out = io.StringIO()
    with redirect_stdout(out):
        run.main()
    text = out.getvalue()
    assert "How many cultures are active? 1" in text
    assert "Ada's open total at position 5: 30" in text
    assert "credit_limit" not in text.split("agent sees:")[1].split("\n")[0]
    assert "agent may not: ['UNKNOWN_CAPABILITY']" in text
