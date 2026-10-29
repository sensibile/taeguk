import sys
from pathlib import Path

base = Path(__file__).parent / "old"
from config import base  # noqa: E402

sys.path.insert(0, str(base))
