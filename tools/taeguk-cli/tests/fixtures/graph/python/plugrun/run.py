import sys
from os.path import dirname, join

sys.path.insert(0, join(dirname(__file__), "..", "plug"))
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent / "pretend"))
