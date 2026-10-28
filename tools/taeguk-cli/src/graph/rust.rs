use super::{Analyzer, Edge, Gap, Node, Partial, exec, normalize, relative, version};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
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
    name: String,
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
    name: String,
    rename: Option<String>,
    path: Option<PathBuf>,
}

/// One module of a crate: the file holding it, child module names and the paths it mentions.
#[derive(Default)]
struct Module {
    file: PathBuf,
    children: BTreeSet<String>,
    paths: Vec<Vec<String>>,
}
type Tree = BTreeMap<Vec<String>, Module>;

pub(super) fn analyze(root: &Path) -> (Analyzer, Result<Partial, String>) {
    let mut a = Analyzer::new("rust", "cargo+syn", &COMMAND);
    a.version = version("cargo", &["--version"])
        .map(|c| format!("{c}; taeguk-cli {} syn 3", env!("CARGO_PKG_VERSION")));
    let result = run(root, &mut a);
    (a, result)
}

fn run(root: &Path, a: &mut Analyzer) -> Result<Partial, String> {
    let o = exec(
        Command::new("cargo")
            .args(&COMMAND[1..])
            .current_dir(root)
            .env("CARGO_NET_OFFLINE", "true"),
    )?;
    a.exit_code = o.code;
    if o.code != Some(0) {
        return Err(format!("cargo metadata failed: {}", o.stderr.trim()));
    }
    let meta: Metadata = serde_json::from_slice(&o.stdout)
        .map_err(|e| format!("cannot parse cargo metadata: {e}"))?;
    let lib_of = |p: &Package| {
        p.targets
            .iter()
            .find(|t| {
                t.kind
                    .iter()
                    .any(|k| k.contains("lib") || k == "proc-macro")
            })
            .map(|t| t.src_path.clone())
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
                if let Some(lib) = lib_of(dep) {
                    externs.insert(
                        d.rename
                            .clone()
                            .unwrap_or_else(|| d.name.clone())
                            .replace('-', "_"),
                        lib,
                    );
                }
            }
            if let Some(lib) = own_lib.as_ref().filter(|l| **l != t.src_path) {
                externs.insert(p.name.replace('-', "_"), lib.clone());
            }
            if !trees.contains_key(&t.src_path) {
                let mut tree = Tree::new();
                let dir = t.src_path.parent().unwrap_or(root).to_path_buf();
                walk_file(&t.src_path, vec![], &dir, &mut tree, &mut out.gaps, root)
                    .map_err(|e| format!("target {}: {e}", t.name))?;
                trees.insert(t.src_path.clone(), tree);
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
                if let Some(target) = resolve(tree, path, segs, externs, &trees)
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
) -> Option<PathBuf> {
    let (mut tree, mut base, mut rest) = (tree, here.to_vec(), segs);
    match segs.first()?.as_str() {
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

fn walk_file(
    file: &Path,
    path: Vec<String>,
    mod_dir: &Path,
    tree: &mut Tree,
    gaps: &mut Vec<Gap>,
    root: &Path,
) -> Result<(), String> {
    let text = fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
    let ast = syn::parse_file(&text).map_err(|e| format!("{}: {e}", file.display()))?;
    let file_dir = file.parent().unwrap_or(root).to_path_buf();
    items(
        &ast.items, path, file, &file_dir, mod_dir, true, tree, gaps, root,
    )
}

#[allow(clippy::too_many_arguments)]
fn items(
    list: &[syn::Item],
    path: Vec<String>,
    file: &Path,
    file_dir: &Path,
    mod_dir: &Path,
    file_level: bool,
    tree: &mut Tree,
    gaps: &mut Vec<Gap>,
    root: &Path,
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
        let attr_path = m
            .attrs
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
            items(
                content, child_path, file, file_dir, &dir, false, tree, gaps, root,
            )?;
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
            let shown = relative(root, file).unwrap_or_else(|| file.display().to_string());
            if m.attrs.iter().any(|a| a.path().is_ident("cfg")) {
                gaps.push(Gap::new(
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
            child.parent().unwrap_or(root).to_path_buf()
        } else {
            child.with_extension("")
        };
        walk_file(&child, child_path, &child_dir, tree, gaps, root)?;
    }
    let m = tree.entry(path).or_default();
    m.file = file.to_path_buf();
    m.children.extend(children);
    m.paths.extend(paths.0);
    Ok(())
}

/// Collects `use` trees and expression/type paths; nested modules are walked separately.
#[derive(Default)]
struct Paths(Vec<Vec<String>>);
impl Paths {
    fn use_tree(&mut self, prefix: &mut Vec<String>, t: &syn::UseTree) {
        match t {
            syn::UseTree::Path(p) => {
                prefix.push(p.ident.to_string());
                self.use_tree(prefix, &p.tree);
                prefix.pop();
            }
            syn::UseTree::Name(n) if n.ident == "self" => self.0.push(prefix.clone()),
            syn::UseTree::Name(n) => self
                .0
                .push([prefix.clone(), vec![n.ident.to_string()]].concat()),
            syn::UseTree::Rename(r) => self
                .0
                .push([prefix.clone(), vec![r.ident.to_string()]].concat()),
            syn::UseTree::Glob(_) => self.0.push(prefix.clone()),
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
        self.0
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
        let got: Vec<String> = p.0.iter().map(|s| s.join("::")).collect();
        assert_eq!(
            got,
            ["crate::a", "crate::a::B", "crate::c", "super::d", "x::y"]
        );
    }
}
