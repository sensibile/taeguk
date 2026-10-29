import sys
from pathlib import Path

p = Path(__file__).parent / "old"
p, x = (Path(__file__).parent / "new", 1)
sys.path.insert(0, str(p))
