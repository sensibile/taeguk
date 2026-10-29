import sys
import pathlib

pathlib.Path = lambda _: "new"
sys.path.insert(0, str(pathlib.Path(__file__)))
