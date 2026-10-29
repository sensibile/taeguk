import builtins
import sys

builtins.__dict__["exec"]("import hidden")
sys.__dict__["path"].insert(0, "extra")
