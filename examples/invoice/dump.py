"""Prints the canonical wire IR of the invoice example (no trailing newline)."""

import sys

from examples.invoice.behavior import model

sys.stdout.write(model.to_wire_json())
