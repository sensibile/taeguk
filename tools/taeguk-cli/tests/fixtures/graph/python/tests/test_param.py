import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1] / "src"


def load(ROOT):
    sys.path.insert(0, str(ROOT))
