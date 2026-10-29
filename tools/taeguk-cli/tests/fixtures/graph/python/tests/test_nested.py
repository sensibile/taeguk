import sys
from pathlib import Path

P = Path(__file__).resolve().parents[1] / "scripts"

if True:

    def load(P):
        sys.path.insert(0, str(P))
