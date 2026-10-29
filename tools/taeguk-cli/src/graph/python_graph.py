"""Static import graph for tracked Python files. Standard library only; reads JSON from stdin."""

import ast
import json
import os
import stat
import sys
import tomllib
from pathlib import PurePosixPath


MAX_SOURCE = 8 << 20


def opened_path(fd):
    """The path of an open file as the kernel reports it."""
    import fcntl
    if hasattr(fcntl, "F_GETPATH"):
        return os.fsdecode(fcntl.fcntl(fd, fcntl.F_GETPATH, bytes(1024)).rstrip(b"\0"))
    return os.readlink(f"/proc/self/fd/{fd}")


def read_source(root, path):
    """Bytes of a file that resolves to a regular file inside ROOT, at most MAX_SOURCE."""
    real = os.path.realpath(os.path.join(root, path))
    if os.path.commonpath([root, real]) != root:
        raise ValueError("resolves outside ROOT")
    # Check before opening: opening a FIFO blocks until a writer appears.
    if not stat.S_ISREG(os.stat(real).st_mode):
        raise ValueError("not a regular file")
    with open(os.open(real, os.O_RDONLY | os.O_NONBLOCK), "rb") as f:
        if not stat.S_ISREG(os.fstat(f.fileno()).st_mode):
            raise ValueError("not a regular file")
        # The opened file itself must be inside ROOT: a file or parent swapped for a link out of
        # ROOT after the check above is refused.
        opened = opened_path(f.fileno())
        if os.path.commonpath([root, opened]) != root:
            raise ValueError("replaced by a link out of ROOT")
        data = f.read(MAX_SOURCE + 1)
    if len(data) > MAX_SOURCE:
        raise ValueError(f"larger than {MAX_SOURCE} bytes")
    return data


def configured_roots(root, files):
    roots = []
    try:
        tool = tomllib.loads(read_source(root, "pyproject.toml").decode()).get("tool", {})
        setuptools = tool.get("setuptools", {})
        found = setuptools.get("packages", {})
        if isinstance(found, dict):
            roots += found.get("find", {}).get("where", [])
        package_dir = setuptools.get("package-dir", {})
        if isinstance(package_dir, dict) and "" in package_dir:
            roots.append(package_dir[""])
    except FileNotFoundError:
        pass
    if any(f.startswith("src/") for f in files):
        roots.append("src")
    roots.append("")
    seen = []
    for r in roots:
        r = str(PurePosixPath(r)).strip("/") if r not in ("", ".") else ""
        if r not in seen:
            seen.append(r)
    return seen


def join(*parts):
    return "/".join(p for p in parts if p)


def candidates(name, roots, files):
    """Files that module `name` resolves to under each root, plus their package __init__ files."""
    parts = name.split(".")
    hits = {}
    for r in roots:
        base = join(r, *parts)
        # A package directory wins over a module file of the same name, as in Python's finder.
        target = next((f for f in (join(base, "__init__.py"), base + ".py") if f in files), None)
        if target:
            inits = [join(r, *parts[:i], "__init__.py") for i in range(1, len(parts))]
            hits[target] = [i for i in inits if i in files]
    return hits


class Unresolved(Exception):
    pass


def norm(parts):
    out = []
    for p in parts:
        if p in ("", "."):
            continue
        if p == "..":
            if not out:
                raise Unresolved("path leaves ROOT")
            out.pop()
        else:
            out.append(p)
    return out


# Per-file context: the ROOT and the names bound to known path functions and modules.
CTX = {"root": "", "funcs": {}, "modules": {}}
PATH_FUNCS = {("pathlib", "Path"): "Path", ("pathlib", "PurePath"): "Path",
              ("pathlib", "PosixPath"): "Path", ("os.path", "dirname"): "dirname",
              ("os.path", "join"): "join", ("os.path", "abspath"): "abspath",
              ("os.path", "realpath"): "realpath", ("os", "fspath"): "fspath"}


def path_bindings(tree, shadowed):
    """Names bound to known path functions and modules, minus any name bound another way.
    Modules in `shadowed` exist as project files, so an import of them may not be the
    standard library; nothing bound from them counts."""
    funcs, modules, other = {"str": "str"}, {}, set()
    for node in ast.walk(tree):
        if isinstance(node, ast.Import):
            for a in node.names:
                if a.name.split(".")[0] in shadowed:
                    other.add((a.asname or a.name).split(".")[0])
                elif a.name in ("os", "os.path", "pathlib") and a.asname:
                    modules[a.asname] = a.name
                elif a.name in ("os", "os.path", "pathlib"):
                    modules[a.name.split(".")[0]] = a.name.split(".")[0]
                else:
                    other.add((a.asname or a.name).split(".")[0])
        elif isinstance(node, ast.ImportFrom):
            for a in node.names:
                bound = a.asname or a.name
                # A relative import (`from .pathlib import Path`) or a module the project itself
                # provides (`scripts/pathlib.py`) is not the standard library.
                own = node.level > 0 or (node.module or "").split(".")[0] in shadowed
                if not own and (node.module, a.name) in PATH_FUNCS:
                    funcs[bound] = PATH_FUNCS[(node.module, a.name)]
                elif not own and node.module == "os" and a.name == "path":
                    modules[bound] = "os.path"
                else:
                    other.add(bound)
        else:
            other.update(bound_names(node))
    CTX["file_rebound"] = "__file__" in other
    # A patched module attribute (`pathlib.Path = fake`) changes what its functions do.
    if module_patched(tree, set(modules) | {"pathlib", "os"}):
        return {}, {}
    # A wildcard import (`from helper import *`) can rebind any name, so no binding is trusted.
    if any(isinstance(n, ast.ImportFrom) and any(a.name == "*" for a in n.names) for n in ast.walk(tree)):
        CTX["file_rebound"] = True
        return {}, {}
    funcs = {k: v for k, v in funcs.items() if k not in other}
    modules = {k: v for k, v in modules.items() if k not in other}
    return funcs, modules


def known_function(func):
    """The known path function a call target is bound to in this file, or None."""
    if isinstance(func, ast.Name):
        return CTX["funcs"].get(func.id)
    if not isinstance(func, ast.Attribute):
        return None
    owner = func.value
    if isinstance(owner, ast.Name):
        module = CTX["modules"].get(owner.id)
    elif isinstance(owner, ast.Attribute) and owner.attr == "path" and isinstance(owner.value, ast.Name):
        module = "os.path" if CTX["modules"].get(owner.value.id) == "os" else None
    else:
        module = None
    return PATH_FUNCS.get((module, func.attr)) if module else None


def real_parts(parts):
    """`resolve()`/`realpath`: follow links inside ROOT; a path that leaves ROOT is unresolved."""
    root = CTX["root"]
    real = os.path.realpath(os.path.join(root, *parts))
    if os.path.commonpath([root, real]) != root:
        raise Unresolved("resolved path leaves ROOT")
    rel = os.path.relpath(real, root)
    return [] if rel == "." else rel.split(os.sep)


def physical(base, more):
    """`base` joined with literal segments as the OS walks them: a `..` right after a
    symbolic link inside ROOT goes to the link target's parent, not back lexically, so such a
    path is unresolved."""
    out = list(base)
    for p in more:
        if p in ("", "."):
            continue
        if p == "..":
            if not out:
                raise Unresolved("path leaves ROOT")
            if os.path.islink(os.path.join(CTX["root"], *out)):
                raise Unresolved("'..' after a symbolic link")
            out.pop()
        else:
            out.append(p)
    return out


def dir_value(node, path, env):
    """Like eval_path, but only a ROOT-relative directory; bare strings depend on the working directory."""
    value = eval_path(node, path, env)
    if not isinstance(value, list):
        raise Unresolved(ast.unparse(node))
    return value


def eval_path(node, path, env):
    """Repository-relative path parts for common `__file__`-based path expressions."""
    if isinstance(node, ast.Name):
        if node.id == "__file__":
            # A module that rebinds __file__ anywhere no longer means its own path by it.
            if CTX.get("file_rebound"):
                raise Unresolved("__file__ is reassigned in this module")
            return norm(path.split("/"))
        if node.id in env:
            return env[node.id]
    elif isinstance(node, ast.Constant) and isinstance(node.value, str) and not node.value.startswith("/"):
        return ("rel", node.value)
    elif isinstance(node, ast.Attribute) and node.attr == "parent":
        return norm(dir_value(node.value, path, env) + [".."])
    elif isinstance(node, ast.Subscript) and isinstance(node.value, ast.Attribute) and node.value.attr == "parents":
        index = node.slice
        if isinstance(index, ast.Constant) and isinstance(index.value, int):
            base = dir_value(node.value.value, path, env)
            # Check the index first so a huge literal cannot allocate a huge list.
            if not 0 <= index.value < len(base):
                raise Unresolved(f"parents[{index.value}] leaves ROOT")
            return base[: len(base) - index.value - 1]
    elif isinstance(node, ast.BinOp) and isinstance(node.op, ast.Div):
        right = eval_path(node.right, path, env)
        if isinstance(right, tuple):
            return physical(dir_value(node.left, path, env), right[1].split("/"))
    elif isinstance(node, ast.Call):
        func = node.func
        if isinstance(func, ast.Attribute) and func.attr in ("resolve", "absolute") and not node.args:
            value = dir_value(func.value, path, env)
            return real_parts(value) if func.attr == "resolve" else value
        # Allow-list: a function counts only when it is bound to the known one in this file
        # (`from os.path import dirname`, `import os` then `os.path.dirname`, builtin `str`)
        # and the name is not rebound anywhere else in the file.
        name = known_function(func)
        if name in ("Path", "str", "abspath", "realpath", "fspath") and len(node.args) == 1:
            value = eval_path(node.args[0], path, env)
            if isinstance(value, list):
                return real_parts(value) if name == "realpath" else value
        if name == "dirname" and len(node.args) == 1:
            return norm(dir_value(node.args[0], path, env) + [".."])
        if name == "join" and node.args:
            parts = dir_value(node.args[0], path, env)
            for arg in node.args[1:]:
                more = eval_path(arg, path, env)
                if not isinstance(more, tuple):
                    raise Unresolved("non-literal join segment")
                parts = physical(parts, more[1].split("/"))
            return parts
    raise Unresolved(ast.unparse(node))


DYNAMIC = ("import_module", "__import__", "spec_from_file_location", "run_path")
# sys attributes besides path that decide or record how imports resolve.
SYS_IMPORT_STATE = ("meta_path", "path_hooks", "path_importer_cache")
DYNAMIC_MODULES = ("importlib", "runpy", "pkgutil", "imp", "zipimport")
# Builtins that run code given as data; what that code imports is not seen statically.
CODE_EXEC = ("exec", "eval", "compile")


def bound_names(node):
    """Names a node binds: assignment and deletion targets, parameters, definitions, imports,
    `except ... as`, and match patterns (`case p`, `case [*rest]`, `case {**rest}`)."""
    if isinstance(node, ast.Name) and isinstance(node.ctx, (ast.Store, ast.Del)):
        return [node.id]
    if isinstance(node, ast.arg):
        return [node.arg]
    if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)):
        return [node.name]
    if isinstance(node, (ast.Global, ast.Nonlocal)):
        return list(node.names)
    if isinstance(node, (ast.MatchAs, ast.MatchStar)) and node.name:
        return [node.name]
    if isinstance(node, ast.MatchMapping) and node.rest:
        return [node.rest]
    if isinstance(node, ast.ExceptHandler) and node.name:
        return [node.name]
    return []


def module_patched(tree, modules):
    """Whether code changes an attribute of one of `modules` (`pathlib.Path = ...`,
    `setattr(os.path, ...)`, `del pathlib.Path`), which makes its functions unknown."""
    def is_module(node):
        if isinstance(node, ast.Name):
            return node.id in modules
        return isinstance(node, ast.Attribute) and is_module(node.value)
    for node in ast.walk(tree):
        if isinstance(node, ast.Attribute) and isinstance(node.ctx, (ast.Store, ast.Del)) and is_module(node.value):
            return True
        if (isinstance(node, ast.Call) and isinstance(node.func, ast.Name) and node.func.id in ("setattr", "delattr")
                and node.args and is_module(node.args[0])):
            return True
    return False


def assigned_names(node):
    """(value, target names) of a plain, annotated or walrus assignment, else (None, [])."""
    if isinstance(node, ast.Assign):
        return node.value, [t.id for t in node.targets if isinstance(t, ast.Name)]
    if isinstance(node, (ast.AnnAssign, ast.NamedExpr)) and isinstance(node.target, ast.Name):
        return node.value, [node.target.id]
    return None, []


def has_wildcard(tree):
    return any(isinstance(n, ast.ImportFrom) and any(a.name == "*" for a in n.names) for n in ast.walk(tree))


def bindings(tree):
    """Names bound to the sys and builtins modules (`import sys as s`, `b = builtins`) and to
    dynamic import functions (`from importlib import import_module as load`)."""
    sys_names, builtins_names, dynamic = {"sys"}, {"builtins", "__builtins__"}, set(DYNAMIC)
    for node in ast.walk(tree):
        if isinstance(node, ast.Import):
            sys_names |= {a.asname for a in node.names if a.name == "sys" and a.asname}
            builtins_names |= {a.asname for a in node.names if a.name == "builtins" and a.asname}
        elif isinstance(node, ast.ImportFrom):
            dynamic |= {a.asname for a in node.names if a.name in DYNAMIC and a.asname}
        value, names = assigned_names(node)
        if value is not None:
            name = getattr(value, "attr", None) or getattr(value, "id", None)
            if isinstance(value, (ast.Name, ast.Attribute)) and name in DYNAMIC:
                dynamic |= set(names)
            if isinstance(value, ast.Name) and value.id in sys_names:
                sys_names |= set(names)
            if isinstance(value, ast.Name) and value.id in builtins_names:
                builtins_names |= set(names)
    return sys_names, builtins_names, dynamic


def parent_map(tree):
    return {id(child): node for node in ast.walk(tree) for child in ast.iter_child_nodes(node)}


def unevaluated_sys_uses(tree, sys_names, parents):
    """Allow-list for the sys module: `sys.path.insert(...)`/`append(...)` calls are evaluated,
    `x in sys.path` only reads, `sys.<other>` is unrelated. Any other use of `sys.path` or of
    `sys` itself (aliased, annotated, passed on, subscripted, assigned) may change the search
    path in a way not evaluated, so each one is unresolved."""
    found = []
    for node in ast.walk(tree):
        if not (isinstance(node, ast.Name) and node.id in sys_names and isinstance(node.ctx, ast.Load)):
            continue
        parent = parents.get(id(node))
        if not (isinstance(parent, ast.Attribute) and parent.value is node):
            found.append(f"line {node.lineno}: {node.id} used as a value")
            continue
        if parent.attr in SYS_IMPORT_STATE:
            found.append(f"line {node.lineno}: sys.{parent.attr} changes how imports resolve")
            continue
        if parent.attr != "path":
            continue
        use = parents.get(id(parent))
        call = parents.get(id(use))
        if (isinstance(use, ast.Attribute) and use.attr in ("insert", "append")
                and isinstance(call, ast.Call) and call.func is use):
            continue
        if isinstance(use, ast.Compare) and all(isinstance(o, (ast.In, ast.NotIn)) for o in use.ops):
            continue
        found.append(f"line {node.lineno}: sys.path used in a form that is not evaluated")
    return found


def is_sys_path(node, sys_names):
    return (isinstance(node, ast.Attribute) and node.attr == "path"
            and isinstance(node.value, ast.Name) and node.value.id in sys_names)


def sys_path_changes(tree, path, sys_names):
    """Directories added to sys.path. Only module-level statements are evaluated, in order, so a
    change sees the path variables as they are at that point. Changes inside functions and
    classes run later with values not known statically (parameters, locals, reassignments), so
    each one is unresolved."""
    env, front, back, unresolved = {}, [], [], []
    # A wildcard import may rebind `sys` or the path variables, so nothing is evaluated.
    wildcard = has_wildcard(tree)
    # Nodes inside any function, lambda or class body, wherever it is nested (`if`, `try` ...).
    inside = {id(n) for f in ast.walk(tree)
              if isinstance(f, (ast.FunctionDef, ast.AsyncFunctionDef, ast.Lambda, ast.ClassDef))
              for n in ast.walk(f) if n is not f}

    def changes(scope, current, deferred, top=None):
        for node in ast.walk(scope):
            # Assignments to sys.path or its slices, other names for it, and `from sys import path`
            # are not evaluated.
            targets = []
            if isinstance(node, ast.Assign):
                targets = node.targets
            elif isinstance(node, (ast.AugAssign, ast.AnnAssign)):
                targets = [node.target]
            if any(is_sys_path(t.value if isinstance(t, ast.Subscript) else t, sys_names) for t in targets):
                unresolved.append(f"line {node.lineno}: sys.path assignment")
            if isinstance(node, ast.ImportFrom) and node.module == "sys":
                taken = [a.name for a in node.names if a.name in ("path", "*") + SYS_IMPORT_STATE]
                if taken:
                    unresolved.append(f"line {node.lineno}: from sys import {', '.join(taken)}")
            if not (isinstance(node, ast.Call) and isinstance(node.func, ast.Attribute)):
                continue
            if not is_sys_path(node.func.value, sys_names):
                continue
            method, args = node.func.attr, node.args
            arg = args[1] if method == "insert" and len(args) == 2 else args[0] if method == "append" and args else None
            try:
                if arg is None:
                    raise Unresolved(f"sys.path.{method}")
                if deferred or id(node) in inside or (top is not None and node is not top.value) or wildcard:
                    # Inside a function or class the values at call time (parameters, locals,
                    # later reassignments) are not known statically.
                    raise Unresolved(f"sys.path.{method} inside a function, class or conditional block")
                value = eval_path(arg, path, current)
                if not isinstance(value, list):
                    raise Unresolved(ast.unparse(arg))
                (front if method == "insert" else back).append("/".join(value))
            except Unresolved as e:
                unresolved.append(f"line {node.lineno}: {e}")

    later = []
    for stmt in tree.body:
        if isinstance(stmt, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)):
            later.append(stmt)
            continue
        # Only statements that always run at that point are evaluated; a change inside
        # `if`/`try`/`for`/`with`/`match` may or may not run.
        # Only a statement that is the call itself (`sys.path.insert(...)`, `x = sys.path.append(...)`)
        # always runs at that point; a call inside `if`/`try`/`for`/`with`/`match`, a boolean or
        # conditional expression or a comprehension may or may not run, so it is not evaluated.
        changes(stmt, env, not isinstance(stmt, (ast.Expr, ast.Assign, ast.AnnAssign)), stmt)
        value = None
        if isinstance(stmt, ast.Assign) and len(stmt.targets) == 1 and isinstance(stmt.targets[0], ast.Name):
            try:
                value = eval_path(stmt.value, path, env)
            except Unresolved:
                value = None
        # Every name the statement binds anywhere (the target, a walrus in the value, tuple
        # unpacking, `for`, `with ... as`, `del`, match patterns) loses its known value; only a
        # plain `name = <evaluated path>` gets a new one.
        for node in ast.walk(stmt):
            for name in bound_names(node):
                env.pop(name, None)
        if isinstance(value, list):
            env[stmt.targets[0].id] = value
    for stmt in later:
        changes(stmt, env, True)
    return front, back, unresolved


def main():
    data = json.load(sys.stdin)
    root, files = data["root"], set(data["files"])
    roots = configured_roots(root, files)
    edges, gaps, errors = set(), [], []
    for path in sorted(files):
        try:
            tree = ast.parse(read_source(root, path), path)
        except (OSError, SyntaxError, ValueError) as e:
            errors.append(f"{path}: {e}")
            continue
        here = str(PurePosixPath(path).parent)
        here = "" if here == "." else here
        is_package = join(here, "__init__.py") in files
        # A script directory without __init__.py is sys.path[0] when the script runs.
        search = roots if is_package else [here] + [r for r in roots if r != here]
        CTX["root"] = root
        # A project module that this file's imports reach under the standard name (a
        # `scripts/pathlib.py` next to the script) replaces the standard one, so none of its
        # names is trusted as a path function.
        shadowed = {top for top in ("os", "pathlib") if candidates(top, search, files)}
        CTX["funcs"], CTX["modules"] = path_bindings(tree, shadowed)
        sys_names, builtins_names, dynamic = bindings(tree)
        parents = parent_map(tree)
        front, back, unresolved = sys_path_changes(tree, path, sys_names)
        unresolved += unevaluated_sys_uses(tree, sys_names, parents)
        search = list(dict.fromkeys(front + search + back))
        for detail in unresolved:
            gaps.append({"kind": "sys_path_unresolved", "file": path,
                         "detail": f"{detail}; imports resolved through it may be missing"})
        for node in ast.walk(tree):
            names = []
            if isinstance(node, ast.Import):
                names = [a.name for a in node.names]
            elif isinstance(node, ast.ImportFrom):
                if node.level:
                    base = PurePosixPath(here or ".")
                    for _ in range(node.level - 1):
                        base = base.parent
                    pkg = "" if str(base) == "." else str(base)
                    parts = node.module.split(".") if node.module else []
                    mod = join(pkg, *parts)
                    # Importing pkg.sub.util also runs pkg/__init__.py and pkg/sub/__init__.py.
                    inits = [join(pkg, *parts[:i], "__init__.py") for i in range(len(parts))]
                    for f in inits:
                        if f in files and f != path:
                            edges.add((path, f))
                    if any(a.name == "*" for a in node.names) and join(mod, "__init__.py") in files:
                        gaps.append({"kind": "dynamic_import", "file": path,
                                     "detail": f"line {node.lineno}: wildcard import from a package may load the submodules its __all__ lists"})
                    for target in ([mod] + [join(mod, a.name) for a in node.names]):
                        # A package directory wins over a module file of the same name.
                        f = next((f for f in (join(target, "__init__.py"), target + ".py") if f in files), None)
                        if f and f != path:
                            edges.add((path, f))
                    continue
                names = [node.module] + [f"{node.module}.{a.name}" for a in node.names if a.name != "*"]
                # `from pkg import *` also imports the submodules `pkg.__all__` names.
                if any(a.name == "*" for a in node.names) and any(
                        t.endswith("__init__.py") for t in candidates(node.module, search, files)):
                    gaps.append({"kind": "dynamic_import", "file": path,
                                 "detail": f"line {node.lineno}: wildcard import from a package may load the submodules its __all__ lists"})
            elif isinstance(node, ast.Call):
                func = node.func
                called = getattr(func, "attr", None) or getattr(func, "id", None)
                if called in dynamic:
                    gaps.append({"kind": "dynamic_import", "file": path,
                                 "detail": f"line {node.lineno}: {called}() target is not resolved statically"})
            # Any reference to the dynamic import machinery marks the file, by name, attribute
            # (`builtins.__import__`) or string (`getattr(builtins, "__import__")`), however the
            # function is later renamed or passed on: the call itself may not be recognizable.
            machinery = None
            if isinstance(node, ast.Import):
                machinery = next((a.name for a in node.names if a.name.split(".")[0] in DYNAMIC_MODULES), None)
            elif isinstance(node, ast.ImportFrom) and node.module and node.module.split(".")[0] in DYNAMIC_MODULES:
                machinery = node.module
            elif isinstance(node, ast.ImportFrom) and node.module in ("builtins", "sys"):
                # `from builtins import exec as run`, `from sys import modules`: the names leave
                # the module, so the allow-list on module attributes no longer sees their uses.
                risky = CODE_EXEC + ("__import__", "*") if node.module == "builtins" else ("modules", "*")
                taken = [a.name for a in node.names if a.name in risky]
                if taken:
                    machinery = f"from {node.module} import {', '.join(taken)}"
            elif isinstance(node, ast.Name) and node.id == "__import__":
                machinery = "__import__"
            elif isinstance(node, ast.Attribute) and node.attr in DYNAMIC:
                machinery = node.attr
            elif isinstance(node, ast.Constant) and node.value in DYNAMIC + DYNAMIC_MODULES:
                machinery = node.value
            elif isinstance(node, ast.Name) and node.id in CODE_EXEC:
                machinery = f"{node.id}()"
            elif isinstance(node, ast.Name) and node.id in builtins_names:
                # Allow-list for the builtins module: only attributes other than code execution
                # and imports pass; the module used as a value may reach them anywhere.
                parent = parents.get(id(node))
                if not (isinstance(parent, ast.Attribute) and parent.value is node):
                    machinery = f"{node.id} used as a value"
                elif parent.attr in CODE_EXEC + ("__import__",):
                    machinery = f"{node.id}.{parent.attr}"
            elif (isinstance(node, ast.Attribute) and node.attr == "modules"
                  and isinstance(node.value, ast.Name) and node.value.id in sys_names):
                machinery = "sys.modules"
            if machinery:
                gaps.append({"kind": "dynamic_import", "file": path,
                             "detail": f"line {node.lineno}: uses {machinery}; modules it loads are not resolved statically"})
            for name in names:
                hits = candidates(name, search, files)
                if len(hits) > 1:
                    gaps.append({"kind": "ambiguous_import", "file": path,
                                 "detail": f"line {node.lineno}: {name} matches {', '.join(sorted(hits))}"})
                    continue
                for target, inits in hits.items():
                    for f in [target] + inits:
                        if f != path:
                            edges.add((path, f))
    json.dump({"roots": roots, "edges": sorted(edges), "gaps": gaps, "errors": errors}, sys.stdout)


main()
