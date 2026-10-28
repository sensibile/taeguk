"""Static import graph for tracked Python files. Standard library only; reads JSON from stdin."""

import ast
import json
import sys
import tomllib
from pathlib import PurePosixPath


def configured_roots(root, files):
    roots = []
    try:
        with open(f"{root}/pyproject.toml", "rb") as f:
            tool = tomllib.load(f).get("tool", {})
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
            return norm(dir_value(node.value.value, path, env) + [".."] * (index.value + 1))
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


def sys_path_changes(tree, path):
    """Directories added to sys.path, evaluated from module-level path variables."""
    env, front, back, unresolved = {}, [], [], []
    for stmt in tree.body:
        if isinstance(stmt, ast.Assign) and len(stmt.targets) == 1 and isinstance(stmt.targets[0], ast.Name):
            try:
                value = eval_path(stmt.value, path, env)
                if isinstance(value, list):
                    env[stmt.targets[0].id] = value
            except Unresolved:
                pass
    for node in ast.walk(tree):
        if not (isinstance(node, ast.Call) and isinstance(node.func, ast.Attribute)):
            continue
        target = node.func.value
        if not (isinstance(target, ast.Attribute) and target.attr == "path"
                and isinstance(target.value, ast.Name) and target.value.id == "sys"):
            continue
        method, args = node.func.attr, node.args
        arg = args[1] if method == "insert" and len(args) == 2 else args[0] if method == "append" and args else None
        try:
            if arg is None:
                raise Unresolved(f"sys.path.{method}")
            value = eval_path(arg, path, env)
            if not isinstance(value, list):
                raise Unresolved(ast.unparse(arg))
            (front if method == "insert" else back).append("/".join(value))
        except Unresolved as e:
            unresolved.append(f"line {node.lineno}: {e}")
    return front, back, unresolved


def main():
    data = json.load(sys.stdin)
    root, files = data["root"], set(data["files"])
    roots = configured_roots(root, files)
    edges, gaps, errors = set(), [], []
    for path in sorted(files):
        try:
            with open(f"{root}/{path}", "rb") as f:
                tree = ast.parse(f.read(), path)
        except (SyntaxError, ValueError) as e:
            errors.append(f"{path}: {e}")
            continue
        here = str(PurePosixPath(path).parent)
        here = "" if here == "." else here
        is_package = join(here, "__init__.py") in files
        # A script directory without __init__.py is sys.path[0] when the script runs.
        search = roots if is_package else [here] + [r for r in roots if r != here]
        front, back, unresolved = sys_path_changes(tree, path)
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
                    # Importing pkg.sub.util also runs pkg/sub/__init__.py.
                    inits = [join(pkg, *parts[:i], "__init__.py") for i in range(1, len(parts))]
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
                if called in ("import_module", "__import__", "spec_from_file_location", "run_path"):
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
