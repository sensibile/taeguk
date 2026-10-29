import sys
from pathlib import Path

p = Path(__file__).parent / "old"
match Path(__file__).parent / "new":
    case p:
        pass
sys.path.insert(0, str(p))
