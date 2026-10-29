import sys
from pathlib import Path

p = Path(__file__).parent / "old"
x = (p := Path(__file__).parent / "new")
sys.path.insert(0, str(p))
