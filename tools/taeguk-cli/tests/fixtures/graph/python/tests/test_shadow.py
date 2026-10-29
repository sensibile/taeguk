import sys
from os.path import dirname


def dirname(p):
    return p.rsplit("/", 2)[0] + "/plugins"


sys.path.append(dirname(__file__))
