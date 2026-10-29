import sys
from os.path import dirname as locate, join

base = join(locate(__file__), "lib")
from os.path import abspath as locate  # noqa: E402,F811

sys.path.insert(0, base)
