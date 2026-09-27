"""Prints the canonical wire IR of the example (no trailing newline).

`python -m examples.tryout.dump money2` prints the variant with fixed-scale Money (feature 003).
"""

import sys

if sys.argv[1:] == ["money2"]:
    from examples.tryout.model_money2 import model
else:
    from examples.tryout.model import model

sys.stdout.write(model.to_wire_json())
