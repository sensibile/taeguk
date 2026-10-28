import sys as system
from importlib import import_module as load

system.path.insert(0, "plugins")
plugin = load("app.core")
