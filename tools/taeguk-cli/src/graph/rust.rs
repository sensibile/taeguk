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

/// One module of a crate: the file holding it, child module names, the paths it mentions
/// and the names its `use` items bind (`use crate::a as b` binds `b` to `crate::a`).
#[derive(Default)]
struct Module {
    file: PathBuf,
    children: BTreeSet<String>,
    paths: Vec<Vec<String>>,
    aliases: BTreeMap<String, Vec<String>>,
}
type Tree = BTreeMap<Vec<String>, Module>;

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
            for segs in &m.paths {
                if let Some(target) = resolve(tree, path, segs, externs, &trees, 0)
                    && target != m.file
                {
                    out.edges.push(Edge::new(
                        &format!("rust:{}", rel(&m.file)?),
                        &format!("rust:{}", rel(&target)?),
                        "use",
                    ));
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

/// Resolves a path mentioned in module `here` to the file of the deepest module it names.
fn resolve(
    tree: &Tree,
    here: &[String],
    segs: &[String],
    externs: &BTreeMap<String, PathBuf>,
    trees: &BTreeMap<PathBuf, Tree>,
    expanded: usize,
) -> Option<PathBuf> {
    let first = segs.first()?;
    // A name bound by `use` stands for its full path; the bound limit stops `use a::b as a` loops.
    let alias = tree
        .get(here)
        .and_then(|m| m.aliases.get(first))
        .filter(|full| expanded < 8 && full.as_slice() != std::slice::from_ref(first));
    if let Some(full) = alias {
        let segs = [full.as_slice(), &segs[1..]].concat();
        return resolve(tree, here, &segs, externs, trees, expanded + 1);
    }
    let (mut tree, mut base, mut rest) = (tree, here.to_vec(), segs);
    match first.as_str() {
        "crate" => (base, rest) = (vec![], &segs[1..]),
        "self" => rest = &segs[1..],
        "super" => {
            while rest.first().is_some_and(|s| s == "super") {
                base.pop()?;
                rest = &rest[1..];
            }
        }
        first if tree.get(here).is_some_and(|m| m.children.contains(first)) => {}
        first => {
            tree = trees.get(externs.get(first)?)?;
            (base, rest) = (vec![], &segs[1..]);
        }
    }
    for s in rest {
        if !tree.get(&base)?.children.contains(s) {
            break;
        }
        base.push(s.clone());
    }
    Some(tree.get(&base)?.file.clone())
}

/// Module-tree walk of one crate; `active` holds the files on the current `mod` chain.
struct Walk<'a> {
    root: &'a Path,
    tree: Tree,
    gaps: &'a mut Vec<Gap>,
    active: Vec<PathBuf>,
}

impl Walk<'_> {
    fn shown(&self, file: &Path) -> String {
        relative(self.root, file).unwrap_or_else(|| file.display().to_string())
    }

    fn file(&mut self, file: &Path, path: Vec<String>, mod_dir: &Path) -> Result<(), String> {
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
        let mut paths = Paths::default();
        let mut children = BTreeSet::new();
        for item in list {
            let syn::Item::Mod(m) = item else {
                paths.visit_item(item);
                continue;
            };
            let name = m.ident.to_string();
            let child_path = [path.clone(), vec![name.clone()]].concat();
            children.insert(name.clone());
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
        m.aliases.extend(paths.aliases);
        Ok(())
    }
}

/// Collects `use` trees and expression/type paths; nested modules are walked separately.
#[derive(Default)]
struct Paths {
    paths: Vec<Vec<String>>,
    aliases: BTreeMap<String, Vec<String>>,
}
impl Paths {
    /// Records a `use` leaf: `ident` is the imported name (`self` for the prefix itself),
    /// `bound` the name it is bound to in this module.
    fn leaf(&mut self, prefix: &[String], ident: &syn::Ident, bound: &syn::Ident) {
        let full = if ident == "self" {
            prefix.to_vec()
        } else {
            [prefix, &[ident.to_string()]].concat()
        };
        let bound = if bound == "self" {
            prefix.last().cloned()
        } else {
            Some(bound.to_string())
        };
        if let Some(bound) = bound.filter(|b| b != "_") {
            self.aliases.insert(bound, full.clone());
        }
        self.paths.push(full);
    }
    fn use_tree(&mut self, prefix: &mut Vec<String>, t: &syn::UseTree) {
        match t {
            syn::UseTree::Path(p) => {
                prefix.push(p.ident.to_string());
                self.use_tree(prefix, &p.tree);
                prefix.pop();
            }
            syn::UseTree::Name(n) => self.leaf(prefix, &n.ident, &n.ident),
            syn::UseTree::Rename(r) => self.leaf(prefix, &r.ident, &r.rename),
            syn::UseTree::Glob(_) => self.paths.push(prefix.clone()),
            syn::UseTree::Group(g) => g.items.iter().for_each(|t| self.use_tree(prefix, t)),
        }
    }
}
impl<'ast> Visit<'ast> for Paths {
    fn visit_item_mod(&mut self, _: &'ast syn::ItemMod) {}
    fn visit_item_use(&mut self, u: &'ast syn::ItemUse) {
        self.use_tree(&mut vec![], &u.tree);
    }
    fn visit_path(&mut self, p: &'ast syn::Path) {
        self.paths
            .push(p.segments.iter().map(|s| s.ident.to_string()).collect());
        visit::visit_path(self, p);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn use_trees_flatten_to_paths() {
        let f = syn::parse_file(
            "use crate::{a::{self, B}, c::*}; use super::d as e; fn f() { x::y(); }",
        )
        .unwrap();
        let mut p = Paths::default();
        f.items.iter().for_each(|i| p.visit_item(i));
        let got: Vec<String> = p.paths.iter().map(|s| s.join("::")).collect();
        assert_eq!(
            got,
            ["crate::a", "crate::a::B", "crate::c", "super::d", "x::y"]
        );
        let aliases: Vec<String> = p
            .aliases
            .iter()
            .map(|(k, v)| format!("{k}={}", v.join("::")))
            .collect();
        assert_eq!(aliases, ["B=crate::a::B", "a=crate::a", "e=super::d"]);
    }
}
