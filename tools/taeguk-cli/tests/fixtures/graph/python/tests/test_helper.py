import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
sys.path.append(str(Path.cwd() / "vendor"))

import helper  # noqa: E402

assert helper.go(1) == 1
