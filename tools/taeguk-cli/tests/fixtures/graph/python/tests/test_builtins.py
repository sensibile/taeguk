import builtins


def invoke(fn):
    return fn("app.core")


invoke(builtins.__import__)
