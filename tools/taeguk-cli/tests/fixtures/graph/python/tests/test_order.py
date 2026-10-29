import sys
from pathlib import Path

P = Path(__file__).resolve().parents[1] / "scripts"
sys.path.insert(0, str(P))
P = Path(__file__).resolve().parents[1] / "src"

import helper  # noqa: E402


def later():
    sys.path.append(str(P))


assert helper.go(1) == 1
