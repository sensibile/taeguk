import sys
from pathlib import Path

FAR = Path(__file__).parents[1000000000]
sys.path.insert(0, str(FAR))
sys.path[:0] = [str(Path.home())]
