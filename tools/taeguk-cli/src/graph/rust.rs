use super::{
    Analyzer, Edge, Gap, Node, Partial, exec, normalize, read_source, relative, tool_version,
};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    process::Command,
};
use syn::visit::{self, Visit};

const COMMAND: [&str; 6] = [
    "cargo",
    "metadata",
    "--format-version",
    "1",
    "--no-deps",
    "--offline",
];

#[derive(Deserialize)]
struct Metadata {
    packages: Vec<Package>,
}
#[derive(Deserialize)]
struct Package {
    manifest_path: PathBuf,
    targets: Vec<Target>,
    dependencies: Vec<Dependency>,
}
#[derive(Deserialize)]
struct Target {
    name: String,
    kind: Vec<String>,
    src_path: PathBuf,
}
#[derive(Deserialize)]
struct Dependency {
    rename: Option<String>,
    path: Option<PathBuf>,
}

/// One module of a crate: the file holding it, its children and the paths it mentions.
/// Children map a name to tree keys, because cfg branches can declare one name more than once
/// (`imp`, `imp#2`, ...).
#[derive(Default)]
struct Module {
    file: PathBuf,
    children: BTreeMap<String, Vec<String>>,
    paths: Vec<PathRef>,
}
type Tree = BTreeMap<Vec<String>, Module>;
/// A mentioned path with `use` aliases already expanded, and the glob imports in its scope.
struct PathRef {
    segs: Vec<String>,
    globs: Vec<Vec<String>>,
}
/// Files one target may expand to; repeated `#[path]` reuse can otherwise grow exponentially.
const MAX_MODULE_FILES: usize = 10_000;

pub(super) fn analyze(root: &Path) -> (Analyzer, Result<Partial, String>) {
    let mut a = Analyzer::new("rust", "cargo+syn", &COMMAND);
    a.version = tool_version(root, "rust");
    let result = run(root, &mut a);
    (a, result)
}

fn run(root: &Path, a: &mut Analyzer) -> Result<Partial, String> {
    let o = exec(
        Command::new("cargo")
            .args(&COMMAND[1..])
            .current_dir(root)
            .env("CARGO_NET_OFFLINE", "true")
            // A rust-toolchain file must not make rustup download a toolchain.
            .env("RUSTUP_AUTO_INSTALL", "0"),
    )?;
    a.exit_code = o.code;
    if o.code != Some(0) {
        return Err(format!("cargo metadata failed: {}", o.stderr.trim()));
    }
    let meta: Metadata = serde_json::from_slice(&o.stdout)
        .map_err(|e| format!("cannot parse cargo metadata: {e}"))?;
    // Code names a library by its target name (or a dependency rename), not the package name.
    let lib_of = |p: &Package| {
        p.targets
            .iter()
            .find(|t| {
                t.kind
                    .iter()
                    .any(|k| k.contains("lib") || k == "proc-macro")
            })
            .map(|t| (t.name.replace('-', "_"), t.src_path.clone()))
    };
    let by_dir: BTreeMap<PathBuf, &Package> = meta
        .packages
        .iter()
        .filter_map(|p| Some((p.manifest_path.parent()?.to_path_buf(), p)))
        .collect();
    let mut out = Partial::default();
    let mut trees: BTreeMap<PathBuf, Tree> = BTreeMap::new();
    let mut crates = Vec::new();
    for p in &meta.packages {
        let own_lib = lib_of(p);
        for t in &p.targets {
            // Workspace crates this target can name: path dependencies and its own library.
            let mut externs = BTreeMap::new();
            for d in &p.dependencies {
                let Some(dep) = d.path.as_ref().and_then(|dir| by_dir.get(dir)) else {
                    continue;
                };
                if let Some((name, lib)) = lib_of(dep) {
                    let name = d
                        .rename
                        .as_ref()
                        .map(|r| r.replace('-', "_"))
                        .unwrap_or(name);
                    externs.insert(name, lib);
                }
            }
            if let Some((name, lib)) = own_lib.as_ref().filter(|(_, l)| *l != t.src_path) {
                externs.insert(name.clone(), lib.clone());
            }
            if !trees.contains_key(&t.src_path) {
                let mut w = Walk {
                    root,
                    tree: Tree::new(),
                    gaps: &mut out.gaps,
                    active: vec![],
                    visits: 0,
                };
                let dir = t.src_path.parent().unwrap_or(root).to_path_buf();
                w.file(&t.src_path, vec![], &dir)
                    .map_err(|e| format!("target {}: {e}", t.name))?;
                trees.insert(t.src_path.clone(), w.tree);
            }
            crates.push((t.src_path.clone(), externs));
        }
    }
    let rel =
        |f: &Path| relative(root, f).ok_or_else(|| format!("{} is outside ROOT", f.display()));
    let mut files = BTreeSet::new();
    for tree in trees.values() {
        for m in tree.values() {
            files.insert(rel(&m.file)?);
        }
    }
    for (crate_root, externs) in &crates {
        let tree = &trees[crate_root];
        for (path, m) in tree {
            for p in &m.paths {
                for target in resolve(tree, path, p, externs, &trees) {
                    if target != m.file {
                        out.edges.push(Edge::new(
                            &format!("rust:{}", rel(&m.file)?),
                            &format!("rust:{}", rel(&target)?),
                            "use",
                        ));
                    }
                }
            }
        }
    }
    out.nodes = files
        .into_iter()
        .map(|f| Node::new("rust", f.clone(), vec![f]))
        .collect();
    Ok(out)
}

/// Files of the deepest modules a path mentioned in module `here` names. A name declared by
/// several cfg branches yields each branch. A first segment nothing else binds is tried under
/// the glob imports in scope, and counts only when it reaches a child of the glob target.
fn resolve(
    tree: &Tree,
    here: &[String],
    p: &PathRef,
    externs: &BTreeMap<String, PathBuf>,
    trees: &BTreeMap<PathBuf, Tree>,
) -> BTreeSet<PathBuf> {
    let mut out = BTreeSet::new();
    if !start(tree, here, &p.segs, externs, trees, &mut out) {
        for g in &p.globs {
            let (mut via, mut target) = (BTreeSet::new(), BTreeSet::new());
            start(
                tree,
                here,
                &[g.as_slice(), &p.segs].concat(),
                externs,
                trees,
                &mut via,
            );
            start(tree, here, g, externs, trees, &mut target);
            out.extend(via.difference(&target).cloned());
        }
    }
    out
}

/// Resolves the first segment and descends; false when the first segment is not bound here.
fn start(
    tree: &Tree,
    here: &[String],
    segs: &[String],
    externs: &BTreeMap<String, PathBuf>,
    trees: &BTreeMap<PathBuf, Tree>,
    out: &mut BTreeSet<PathBuf>,
) -> bool {
    let Some(first) = segs.first() else {
        return false;
    };
    let (mut tree, mut base, mut rest) = (tree, here.to_vec(), segs);
    match first.as_str() {
        "crate" => (base, rest) = (vec![], &segs[1..]),
        "self" => rest = &segs[1..],
        "super" => {
            while rest.first().is_some_and(|s| s == "super") {
                if base.pop().is_none() {
                    return false;
                }
                rest = &rest[1..];
            }
        }
        first
            if tree
                .get(here)
                .is_some_and(|m| m.children.contains_key(first)) => {}
        first => match externs.get(first).and_then(|f| trees.get(f)) {
            Some(t) => (tree, base, rest) = (t, vec![], &segs[1..]),
            None => return false,
        },
    }
    descend(tree, base, rest, out);
    true
}

fn descend(tree: &Tree, base: Vec<String>, rest: &[String], out: &mut BTreeSet<PathBuf>) {
    let Some(m) = tree.get(&base) else {
        return;
    };
    match rest.first().and_then(|s| m.children.get(s)) {
        Some(ids) => {
            for id in ids {
                descend(
                    tree,
                    [base.clone(), vec![id.clone()]].concat(),
                    &rest[1..],
                    out,
                );
            }
        }
        None => {
            out.insert(m.file.clone());
        }
    }
}

/// Module-tree walk of one crate; `active` holds the files on the current `mod` chain.
struct Walk<'a> {
    root: &'a Path,
    tree: Tree,
    gaps: &'a mut Vec<Gap>,
    active: Vec<PathBuf>,
    visits: usize,
}

impl Walk<'_> {
    fn shown(&self, file: &Path) -> String {
        relative(self.root, file).unwrap_or_else(|| file.display().to_string())
    }

    fn file(&mut self, file: &Path, path: Vec<String>, mod_dir: &Path) -> Result<(), String> {
        self.visits += 1;
        if self.visits > MAX_MODULE_FILES {
            return Err(format!(
                "module expansion exceeds {MAX_MODULE_FILES} files at {}",
                self.shown(file)
            ));
        }
        let (real, text) = read_source(self.root, file)?;
        // `#[path]` can point a module back at a file on its own chain, which would never end.
        if self.active.contains(&real) {
            return Err(format!(
                "{}: module cycle through #[path]",
                self.shown(file)
            ));
        }
        let ast = syn::parse_file(&text).map_err(|e| format!("{}: {e}", self.shown(file)))?;
        let file_dir = file.parent().unwrap_or(self.root).to_path_buf();
        self.active.push(real);
        let result = self.items(&ast.items, path, file, &file_dir, mod_dir, true);
        self.active.pop();
        result
    }

    fn items(
        &mut self,
        list: &[syn::Item],
        path: Vec<String>,
        file: &Path,
        file_dir: &Path,
        mod_dir: &Path,
        file_level: bool,
    ) -> Result<(), String> {
        let mut paths = Paths::new(list);
        let mut children: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for item in list {
            let syn::Item::Mod(m) = item else {
                paths.visit_item(item);
                continue;
            };
            let name = m.ident.to_string();
            let ids = children.entry(name.clone()).or_default();
            let id = match ids.len() {
                0 => name.clone(),
                n => format!("{name}#{}", n + 1),
            };
            ids.push(id.clone());
            let child_path = [path.clone(), vec![id]].concat();
            let attr_path =
                m.attrs
                    .iter()
                    .find(|a| a.path().is_ident("path"))
                    .and_then(|a| match &a.meta {
                        syn::Meta::NameValue(nv) => match &nv.value {
                            syn::Expr::Lit(syn::ExprLit {
                                lit: syn::Lit::Str(s),
                                ..
                            }) => Some(s.value()),
                            _ => None,
                        },
                        _ => None,
                    });
            if let Some((_, content)) = &m.content {
                let dir = match &attr_path {
                    Some(p) => mod_dir.join(p),
                    None => mod_dir.join(&name),
                };
                self.items(content, child_path, file, file_dir, &dir, false)?;
                continue;
            }
            let found = match &attr_path {
                Some(p) => Some(normalize(&if file_level {
                    file_dir.join(p)
                } else {
                    mod_dir.join(p)
                }))
                .filter(|f| f.is_file()),
                None => [
                    mod_dir.join(format!("{name}.rs")),
                    mod_dir.join(&name).join("mod.rs"),
                ]
                .into_iter()
                .find(|f| f.is_file()),
            };
            let Some(child) = found else {
                let shown = self.shown(file);
                if m.attrs.iter().any(|a| a.path().is_ident("cfg")) {
                    self.gaps.push(Gap::new(
                        "rust",
                        "cfg_module_missing",
                        Some(&shown),
                        format!("mod {name} is cfg-gated and its file was not found"),
                    ));
                    continue;
                }
                return Err(format!("{shown}: file for mod {name} not found"));
            };
            let child_dir = if child.file_name().is_some_and(|n| n == "mod.rs") {
                child.parent().unwrap_or(self.root).to_path_buf()
            } else {
                child.with_extension("")
            };
            self.file(&child, child_path, &child_dir)?;
        }
        let m = self.tree.entry(path).or_default();
        m.file = file.to_path_buf();
        m.children.extend(children);
        m.paths.extend(paths.paths);
        Ok(())
    }
}

/// Names one scope's `use` items bind (`use crate::a as b` binds `b` to `crate::a`) and its
/// glob imports. Items in a module or block are visible in all of it, so a scope is filled
/// before its paths are visited.
#[derive(Default)]
struct Scope {
    aliases: BTreeMap<String, Vec<String>>,
    globs: Vec<Vec<String>>,
}
impl Scope {
    fn of<'a>(items: impl Iterator<Item = &'a syn::Item>) -> Self {
        let mut scope = Scope::default();
        for i in items {
            if let syn::Item::Use(u) = i {
                scope.bind(&mut vec![], &u.tree);
            }
        }
        scope
    }
    fn bind(&mut self, prefix: &mut Vec<String>, t: &syn::UseTree) {
        let mut leaf = |ident: &syn::Ident, bound: &syn::Ident| {
            let full = if ident == "self" {
                prefix.clone()
            } else {
                [prefix.as_slice(), &[ident.to_string()]].concat()
            };
            let bound = if bound == "self" {
                prefix.last().cloned()
            } else {
                Some(bound.to_string())
            };
            if let Some(bound) = bound.filter(|b| b != "_") {
                self.aliases.insert(bound, full);
            }
        };
        match t {
            syn::UseTree::Path(p) => {
                prefix.push(p.ident.to_string());
                self.bind(prefix, &p.tree);
                prefix.pop();
            }
            syn::UseTree::Name(n) => leaf(&n.ident, &n.ident),
            syn::UseTree::Rename(r) => leaf(&r.ident, &r.rename),
            syn::UseTree::Glob(_) => self.globs.push(prefix.clone()),
            syn::UseTree::Group(g) => g.items.iter().for_each(|t| self.bind(prefix, t)),
        }
    }
}

/// Collects `use` trees and expression/type paths of one module, expanding aliases by the
/// scopes they appear in; nested modules are walked separately.
struct Paths {
    paths: Vec<PathRef>,
    scopes: Vec<Scope>,
}
impl Paths {
    fn new(items: &[syn::Item]) -> Self {
        Paths {
            paths: vec![],
            scopes: vec![Scope::of(items.iter())],
        }
    }
    /// Replaces a leading bound name by its path, innermost scope first. A binding whose path
    /// starts with its own name (`use fx_core;`) names the outer item, so expansion stops there.
    fn expand(&self, mut segs: Vec<String>) -> Vec<String> {
        for _ in 0..8 {
            let Some(first) = segs.first() else { break };
            let Some(full) = self.scopes.iter().rev().find_map(|s| s.aliases.get(first)) else {
                break;
            };
            if full.first() == Some(first) {
                break;
            }
            segs = [full.as_slice(), &segs[1..]].concat();
        }
        segs
    }
    fn push(&mut self, segs: Vec<String>) {
        let segs = self.expand(segs);
        let globs = self
            .scopes
            .iter()
            .flat_map(|s| s.globs.iter().map(|g| self.expand(g.clone())))
            .collect();
        self.paths.push(PathRef { segs, globs });
    }
    fn use_tree(&mut self, prefix: &mut Vec<String>, t: &syn::UseTree) {
        match t {
            syn::UseTree::Path(p) => {
                prefix.push(p.ident.to_string());
                self.use_tree(prefix, &p.tree);
                prefix.pop();
            }
            syn::UseTree::Name(syn::UseName { ident })
            | syn::UseTree::Rename(syn::UseRename { ident, .. }) => {
                if ident == "self" {
                    self.push(prefix.clone());
                } else {
                    self.push([prefix.as_slice(), &[ident.to_string()]].concat());
                }
            }
            syn::UseTree::Glob(_) => self.push(prefix.clone()),
            syn::UseTree::Group(g) => g.items.iter().for_each(|t| self.use_tree(prefix, t)),
        }
    }
}
impl<'ast> Visit<'ast> for Paths {
    fn visit_item_mod(&mut self, _: &'ast syn::ItemMod) {}
    fn visit_item_use(&mut self, u: &'ast syn::ItemUse) {
        self.use_tree(&mut vec![], &u.tree);
    }
    fn visit_block(&mut self, b: &'ast syn::Block) {
        self.scopes
            .push(Scope::of(b.stmts.iter().filter_map(|s| match s {
                syn::Stmt::Item(i) => Some(i),
                _ => None,
            })));
        visit::visit_block(self, b);
        self.scopes.pop();
    }
    fn visit_path(&mut self, p: &'ast syn::Path) {
        self.push(p.segments.iter().map(|s| s.ident.to_string()).collect());
        visit::visit_path(self, p);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn paths(src: &str) -> Vec<String> {
        let f = syn::parse_file(src).unwrap();
        let mut p = Paths::new(&f.items);
        f.items.iter().for_each(|i| p.visit_item(i));
        p.paths.iter().map(|s| s.segs.join("::")).collect()
    }
    #[test]
    fn use_trees_flatten_to_paths() {
        assert_eq!(
            paths("use crate::{a::{self, B}, c::*}; use super::d as e; fn f() { x::y(); e::z(); }"),
            [
                "crate::a",
                "crate::a::B",
                "crate::c",
                "super::d",
                "x::y",
                "super::d::z"
            ]
        );
    }
    #[test]
    fn aliases_follow_their_scope() {
        let got = paths(
            "fn one() { use crate::a as x; x::f(); } fn two() { use crate::b as x; x::g(); } \
             use crate::c as y; use y::sub as z; fn three() { z::h(); } use fx_core; fn four() { fx_core::i(); }",
        );
        for want in [
            "crate::a::f",
            "crate::b::g",
            "crate::c::sub::h",
            "fx_core::i",
        ] {
            assert!(got.iter().any(|p| p == want), "{want} not in {got:?}");
        }
        assert!(!got.iter().any(|p| p == "crate::b::f"), "{got:?}");
    }
}
