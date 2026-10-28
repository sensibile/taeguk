"""Static import graph for tracked Python files. Standard library only; reads JSON from stdin."""

import ast
import json
import os
import stat
import sys
import tomllib
from pathlib import PurePosixPath


MAX_SOURCE = 8 << 20


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
        target = next((f for f in (base + ".py", join(base, "__init__.py")) if f in files), None)
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
            return norm(dir_value(node.left, path, env) + right[1].split("/"))
    elif isinstance(node, ast.Call):
        func = node.func
        name = func.attr if isinstance(func, ast.Attribute) else getattr(func, "id", None)
        if name in ("resolve", "absolute") and isinstance(func, ast.Attribute) and not node.args:
            return eval_path(func.value, path, env)
        if name in ("Path", "PurePath", "PosixPath", "str", "abspath", "realpath", "fspath") and len(node.args) == 1:
            value = eval_path(node.args[0], path, env)
            if isinstance(value, list):
                return value
        if name == "dirname" and len(node.args) == 1:
            return norm(dir_value(node.args[0], path, env) + [".."])
        if name == "join" and node.args:
            parts = dir_value(node.args[0], path, env)
            for arg in node.args[1:]:
                more = eval_path(arg, path, env)
                if not isinstance(more, tuple):
                    raise Unresolved("non-literal join segment")
                parts = norm(parts + more[1].split("/"))
            return parts
    raise Unresolved(ast.unparse(node))


DYNAMIC = ("import_module", "__import__", "spec_from_file_location", "run_path")


def bindings(tree):
    """Names bound to the sys module (`import sys as s`) and to dynamic import functions
    (`from importlib import import_module as load`, `load = importlib.import_module`)."""
    sys_names, dynamic = {"sys"}, set(DYNAMIC)
    for node in ast.walk(tree):
        if isinstance(node, ast.Import):
            sys_names |= {a.asname for a in node.names if a.name == "sys" and a.asname}
        elif isinstance(node, ast.ImportFrom):
            dynamic |= {a.asname for a in node.names if a.name in DYNAMIC and a.asname}
        elif isinstance(node, ast.Assign):
            value = node.value
            name = getattr(value, "attr", None) or getattr(value, "id", None)
            if isinstance(value, (ast.Name, ast.Attribute)) and name in DYNAMIC:
                dynamic |= {t.id for t in node.targets if isinstance(t, ast.Name)}
            if isinstance(value, ast.Name) and value.id in sys_names:
                sys_names |= {t.id for t in node.targets if isinstance(t, ast.Name)}
    return sys_names, dynamic


def is_sys_path(node, sys_names):
    return (isinstance(node, ast.Attribute) and node.attr == "path"
            and isinstance(node.value, ast.Name) and node.value.id in sys_names)


def sys_path_changes(tree, path, sys_names):
    """Directories added to sys.path. Module-level statements are evaluated in order, so a
    change sees the path variables as they are at that point. Changes inside functions and
    classes run later; they use the final values, unless a variable they use is assigned more
    than once at module level, which leaves the value unknown."""
    env, front, back, unresolved = {}, [], [], []
    assigned = {}
    for stmt in tree.body:
        for node in ast.walk(stmt):
            if isinstance(node, (ast.Assign, ast.AugAssign, ast.AnnAssign)):
                for t in node.targets if isinstance(node, ast.Assign) else [node.target]:
                    if isinstance(t, ast.Name):
                        assigned[t.id] = assigned.get(t.id, 0) + 1

    def changes(scope, current, deferred):
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
            if isinstance(node, ast.ImportFrom) and node.module == "sys" and any(a.name == "path" for a in node.names):
                unresolved.append(f"line {node.lineno}: from sys import path")
            if isinstance(node, ast.Assign) and is_sys_path(node.value, sys_names):
                unresolved.append(f"line {node.lineno}: sys.path bound to another name")
            if not (isinstance(node, ast.Call) and isinstance(node.func, ast.Attribute)):
                continue
            if not is_sys_path(node.func.value, sys_names):
                continue
            method, args = node.func.attr, node.args
            arg = args[1] if method == "insert" and len(args) == 2 else args[0] if method == "append" and args else None
            try:
                if arg is None:
                    raise Unresolved(f"sys.path.{method}")
                if deferred:
                    names = [n.id for n in ast.walk(arg) if isinstance(n, ast.Name) and assigned.get(n.id, 0) > 1]
                    if names:
                        raise Unresolved(f"{names[0]} is reassigned at module level")
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
        changes(stmt, env, False)
        if isinstance(stmt, ast.Assign) and len(stmt.targets) == 1 and isinstance(stmt.targets[0], ast.Name):
            name = stmt.targets[0].id
            try:
                value = eval_path(stmt.value, path, env)
            except Unresolved:
                value = None
            if isinstance(value, list):
                env[name] = value
            else:
                env.pop(name, None)
        else:
            for node in ast.walk(stmt):
                for t in getattr(node, "targets", None) or [getattr(node, "target", None)]:
                    if isinstance(t, ast.Name):
                        env.pop(t.id, None)
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
        sys_names, dynamic = bindings(tree)
        front, back, unresolved = sys_path_changes(tree, path, sys_names)
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
                    for target in ([mod] + [join(mod, a.name) for a in node.names]):
                        for f in (target + ".py", join(target, "__init__.py")):
                            if f in files and f != path:
                                edges.add((path, f))
                    continue
                names = [node.module] + [f"{node.module}.{a.name}" for a in node.names]
            elif isinstance(node, ast.Call):
                func = node.func
                called = getattr(func, "attr", None) or getattr(func, "id", None)
                if called in dynamic:
                    gaps.append({"kind": "dynamic_import", "file": path,
                                 "detail": f"line {node.lineno}: {called}() target is not resolved statically"})
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
