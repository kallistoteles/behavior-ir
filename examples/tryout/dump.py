"""Prints the canonical wire IR of the example (no trailing newline)."""

import sys

from examples.tryout.model import model

sys.stdout.write(model.to_wire_json())
