"""Prints the canonical wire IR of the example (no trailing newline).

`python -m examples.invoice.dump money2` prints the variant with fixed-scale Money (feature 003).
"""

import sys

if sys.argv[1:] == ["money2"]:
    from examples.invoice.behavior_money2 import model
else:
    from examples.invoice.behavior import model

sys.stdout.write(model.to_wire_json())
