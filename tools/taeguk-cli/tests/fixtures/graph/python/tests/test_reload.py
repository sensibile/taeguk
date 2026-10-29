from importlib import import_module as load

loader = load
plugin = loader("app.core")
