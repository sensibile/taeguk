import importlib

import app.core


def main():
    plugin = importlib.import_module("app.plugin")
    return app.core.X, plugin
