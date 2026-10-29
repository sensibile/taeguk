use super::{
    Analyzer, Edge, Gap, Node, Partial, TempDir, check_source, exec, normalize, read_source,
    relative, tool_version,
};
use serde::Deserialize;
use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    process::Command,
};
use syn::visit::{self, Visit};

const COMMAND: [&str; 8] = [
    "cargo",
    "metadata",
    "--format-version",
    "1",
    "--no-deps",
    "--offline",
    "--manifest-path",
    "<ROOT>/Cargo.toml",
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
/// `exports` holds the module-level `use` bindings, which other modules can reach through it
/// (`pub use crate::imp as facade;` makes `crate::facade::sub` mean `crate::imp::sub`).
#[derive(Default)]
struct Module {
    file: PathBuf,
    children: BTreeMap<String, Vec<String>>,
    paths: Vec<PathRef>,
    exports: Scope,
    /// Standard-named macros used here, `macro_rules!` names defined here, and whether a
    /// `#[macro_use] extern crate` imports macros of unknown names: a crate-wide check turns
    /// a standard name defined or imported anywhere in the crate into a gap.
    plain_macros: BTreeSet<String>,
    defined_macros: BTreeSet<String>,
    imports_macros: bool,
}
type Tree = BTreeMap<Vec<String>, Module>;
/// A mentioned path with `use` aliases already expanded, and the glob imports in its scope.
struct PathRef {
    segs: Vec<String>,
    globs: Vec<Vec<String>>,
}
/// Files one target may expand to; repeated `#[path]` reuse can otherwise grow exponentially.
const MAX_MODULE_FILES: usize = 10_000;
/// Module nesting the walk follows; a longer `#[path]` chain would exhaust the stack.
const MAX_MODULE_DEPTH: usize = 64;
/// Bounds on alias chains and on the paths one mention may expand to (cfg branches multiply).
const MAX_ALIAS_DEPTH: usize = 8;
const MAX_EXPANSIONS: usize = 16;
/// Expansion calls one binding or mention may make.
const MAX_EXPAND_STEPS: usize = 1_024;
/// Module states one mention may visit while following re-exports.
const MAX_RESOLVE_STEPS: usize = 4_096;

pub(super) fn analyze(root: &Path) -> (Analyzer, Result<Partial, String>) {
    let mut a = Analyzer::new("rust", "cargo+syn", &COMMAND);
    a.version = tool_version("rust");
    let result = run(root, &mut a);
    (a, result)
}

fn run(root: &Path, a: &mut Analyzer) -> Result<Partial, String> {
    // cargo runs from a directory outside ROOT: rustup picks the toolchain and cargo reads its
    // configuration by the working directory, so a `rust-toolchain.toml` (which can name any
    // toolchain path, i.e. any program) or `.cargo/config.toml` in the target repository is
    // never consulted.
    let dir = TempDir::new("cargo")?;
    let o = exec(
        Command::new("cargo")
            .args(&COMMAND[1..7])
            .arg(root.join("Cargo.toml"))
            .current_dir(&dir.0)
            .env("CARGO_NET_OFFLINE", "true")
            .env("RUSTUP_AUTO_INSTALL", "0"),
    )?;
    a.exit_code = o.code;
    if o.code != Some(0) {
        return Err(format!("cargo metadata failed: {}", o.stderr.trim()));
    }
    let meta: Metadata = serde_json::from_slice(&o.stdout)
        .map_err(|e| format!("cannot parse cargo metadata: {e}"))?;
    // Manifests cargo read must be inside ROOT, also when reached through a linked directory.
    for p in &meta.packages {
        check_source(root, &p.manifest_path)?;
    }
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
                shadowed_macros(&w.tree, w.gaps, root);
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
    // Each crate's dependency names, so a re-export is followed in the crate that declares it.
    let mut externs_of = BTreeMap::new();
    for (crate_root, externs) in &crates {
        externs_of
            .entry(crate_root.clone())
            .or_insert_with(|| externs.clone());
    }
    let r = Resolver {
        trees: &trees,
        externs_of: &externs_of,
        truncated: Cell::new(false),
        budget: Cell::new(0),
        seen: RefCell::new(BTreeSet::new()),
    };
    for crate_root in externs_of.keys() {
        for (path, m) in &trees[crate_root] {
            r.truncated.set(false);
            for p in &m.paths {
                for target in r.resolve(crate_root, path, p) {
                    if target != m.file {
                        out.edges.push(Edge::new(
                            &format!("rust:{}", rel(&m.file)?),
                            &format!("rust:{}", rel(&target)?),
                            "use",
                        ));
                    }
                }
            }
            if r.truncated.get() {
                out.gaps.push(Gap::new(
                    "rust",
                    "resolution_truncated",
                    Some(&rel(&m.file)?),
                    format!(
                        "re-export chains deeper than {MAX_ALIAS_DEPTH} levels or {MAX_RESOLVE_STEPS} steps were not followed"
                    ),
                ));
            }
        }
    }
    // Module files (`#[path]` may name any extension) and manifests join the input fingerprint.
    out.read.extend(files.iter().cloned());
    for p in &meta.packages {
        out.read.push(rel(&p.manifest_path)?);
    }
    out.nodes = files
        .into_iter()
        .map(|f| Node::new("rust", f.clone(), vec![f]))
        .collect();
    Ok(out)
}

/// A resolution state: crate root, module key and the path segments still to resolve.
type State = (PathBuf, Vec<String>, Vec<String>);

/// A standard macro name is trusted only if nothing in the crate can redefine it: a
/// `macro_rules!` of that name anywhere in the crate, or a `#[macro_use] extern crate`, makes
/// each module that uses the name a gap.
fn shadowed_macros(tree: &Tree, gaps: &mut Vec<Gap>, root: &Path) {
    let defined: BTreeSet<&String> = tree.values().flat_map(|m| &m.defined_macros).collect();
    let imports = tree.values().any(|m| m.imports_macros);
    for m in tree.values() {
        let hit: Vec<String> = m
            .plain_macros
            .iter()
            .filter(|n| imports || defined.contains(n))
            .map(|n| {
                if n.starts_with("derive(") {
                    format!("#[{n}]")
                } else {
                    format!("{n}!")
                }
            })
            .collect();
        if !hit.is_empty() {
            let shown = relative(root, &m.file).unwrap_or_else(|| m.file.display().to_string());
            gaps.push(Gap::new(
                "rust",
                "construct_not_analyzed",
                Some(&shown),
                format!(
                    "{} may be redefined in this crate; its expansion is not analyzed",
                    hit.join(", ")
                ),
            ));
        }
    }
}

/// Resolves mentioned paths to module files. Trees and dependency names are keyed by crate
/// root, so following a path into another crate also switches to that crate's dependencies.
/// `truncated` records that a depth bound cut a resolution short.
struct Resolver<'a> {
    trees: &'a BTreeMap<PathBuf, Tree>,
    externs_of: &'a BTreeMap<PathBuf, BTreeMap<String, PathBuf>>,
    truncated: Cell<bool>,
    /// Steps left for the current mention and the states it already visited: glob re-exports
    /// between modules can otherwise revisit the same state exponentially often.
    budget: Cell<usize>,
    seen: RefCell<BTreeSet<State>>,
}

impl Resolver<'_> {
    /// Files of the deepest modules a path mentioned in module `here` names. A name declared by
    /// several cfg branches yields each branch. A first segment nothing else binds is tried
    /// under the glob imports in scope, and counts only when it reaches a child of the glob target.
    fn resolve(&self, krate: &Path, here: &[String], p: &PathRef) -> BTreeSet<PathBuf> {
        self.budget.set(MAX_RESOLVE_STEPS);
        self.seen.borrow_mut().clear();
        let mut out = BTreeSet::new();
        if !self.start(krate, here, &p.segs, 0, &mut out) {
            for g in &p.globs {
                self.through_glob(krate, here, g, &p.segs, 0, &mut out);
            }
        }
        out
    }

    /// Adds what `rest` reaches below glob target `g` (relative to `here`), not `g` itself.
    fn through_glob(
        &self,
        krate: &Path,
        here: &[String],
        g: &[String],
        rest: &[String],
        depth: usize,
        out: &mut BTreeSet<PathBuf>,
    ) {
        let (mut via, mut target) = (BTreeSet::new(), BTreeSet::new());
        self.start(krate, here, &[g, rest].concat(), depth, &mut via);
        self.start(krate, here, g, depth, &mut target);
        out.extend(via.difference(&target).cloned());
    }

    /// Resolves the first segment and descends; false when the first segment is not bound here.
    fn start(
        &self,
        krate: &Path,
        here: &[String],
        segs: &[String],
        depth: usize,
        out: &mut BTreeSet<PathBuf>,
    ) -> bool {
        let (Some(first), Some(tree)) = (segs.first(), self.trees.get(krate)) else {
            return false;
        };
        let (mut krate, mut base, mut rest) = (krate, here.to_vec(), segs);
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
            first => match self
                .externs_of
                .get(krate)
                .and_then(|e| e.get(first))
                .filter(|f| self.trees.contains_key(*f))
            {
                Some(other) => (krate, base, rest) = (other, vec![], &segs[1..]),
                None => return false,
            },
        }
        self.descend(krate, base, rest, depth, out);
        true
    }

    fn descend(
        &self,
        krate: &Path,
        base: Vec<String>,
        rest: &[String],
        depth: usize,
        out: &mut BTreeSet<PathBuf>,
    ) {
        let Some(m) = self.trees.get(krate).and_then(|t| t.get(&base)) else {
            return;
        };
        let state = (krate.to_path_buf(), base.clone(), rest.to_vec());
        if !self.seen.borrow_mut().insert(state) {
            return;
        }
        if self.budget.get() == 0 {
            self.truncated.set(true);
            return;
        }
        self.budget.set(self.budget.get() - 1);
        let Some(s) = rest.first() else {
            out.insert(m.file.clone());
            return;
        };
        if let Some(ids) = m.children.get(s) {
            for id in ids {
                let child = [base.clone(), vec![id.clone()]].concat();
                self.descend(krate, child, &rest[1..], depth, out);
            }
            return;
        }
        out.insert(m.file.clone());
        let next = m.exports.aliases.contains_key(s) || !m.exports.globs.is_empty();
        if depth >= MAX_ALIAS_DEPTH {
            if next {
                self.truncated.set(true);
            }
            return;
        }
        // A name the module binds with `use` continues at the bound path (re-exports).
        if let Some(fulls) = m.exports.aliases.get(s) {
            for f in fulls {
                self.start(krate, &base, &[f, &rest[1..]].concat(), depth + 1, out);
            }
        } else {
            for g in &m.exports.globs {
                self.through_glob(krate, &base, g, rest, depth + 1, out);
            }
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
        let inner: Vec<String> = ast
            .attrs
            .iter()
            .filter_map(|a| unanalyzed_attribute(&a.meta))
            .map(|n| format!("#![{n}]"))
            .collect();
        if !inner.is_empty() {
            let shown = self.shown(file);
            self.gaps.push(Gap::new(
                "rust",
                "construct_not_analyzed",
                Some(&shown),
                format!(
                    "code generated or hidden by {} is not analyzed",
                    inner.join(", ")
                ),
            ));
        }
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
        if path.len() > MAX_MODULE_DEPTH {
            return Err(format!(
                "{}: modules nested deeper than {MAX_MODULE_DEPTH}",
                self.shown(file)
            ));
        }
        for item in list {
            let syn::Item::Mod(m) = item else {
                paths.visit_item(item);
                continue;
            };
            for a in &m.attrs {
                if let Some(n) = unanalyzed_attribute(&a.meta) {
                    paths.unanalyzed.insert(format!("#[{n}]"));
                }
            }
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
            // `cfg_attr(..., path = "...")` picks another file under some configurations.
            let conditional_path = m
                .attrs
                .iter()
                .any(|a| a.path().is_ident("cfg_attr") && mentions_path_attribute(&a.meta));
            if conditional_path {
                let shown = self.shown(file);
                self.gaps.push(Gap::new(
                    "rust",
                    "cfg_attr_path",
                    Some(&shown),
                    format!("mod {name} has a cfg_attr path; only its default file was analyzed"),
                ));
            }
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
        m.exports = paths.scopes.swap_remove(0);
        m.plain_macros.extend(paths.plain_macros);
        m.defined_macros.extend(paths.defined_macros);
        m.imports_macros |= paths.imports_macros;
        if !paths.unanalyzed.is_empty() {
            let shown = self.shown(file);
            let names: Vec<&str> = paths.unanalyzed.iter().map(String::as_str).collect();
            self.gaps.push(Gap::new(
                "rust",
                "construct_not_analyzed",
                Some(&shown),
                format!(
                    "code generated or hidden by {} is not analyzed",
                    names.join(", ")
                ),
            ));
        }
        if paths.dropped.get() {
            let shown = self.shown(file);
            self.gaps.push(Gap::new(
                "rust",
                "resolution_truncated",
                Some(&shown),
                format!(
                    "use bindings beyond {MAX_EXPANSIONS} paths or {MAX_ALIAS_DEPTH} levels were not expanded"
                ),
            ));
        }
        Ok(())
    }
}

/// Names one scope's `use` items bind (`use crate::a as b` binds `b` to `crate::a`) and its
/// glob imports. A name can have several bindings when cfg branches bind it differently.
/// Items in a module or block are visible in all of it, so a scope is filled before its paths
/// are visited.
#[derive(Default)]
struct Scope {
    aliases: BTreeMap<String, Vec<Vec<String>>>,
    globs: Vec<Vec<String>>,
}
impl Scope {
    fn of<'a>(items: impl Iterator<Item = &'a syn::Item>, dropped: &Cell<bool>) -> Self {
        let mut scope = Scope::default();
        for i in items {
            match i {
                syn::Item::Use(u) => scope.bind(&mut vec![], &u.tree, dropped),
                // `extern crate fx_core as c;` binds `c` like `use fx_core as c;`.
                syn::Item::ExternCrate(e) => {
                    let name = if e.ident == "self" {
                        "crate".to_string()
                    } else {
                        e.ident.to_string()
                    };
                    let bound = e
                        .rename
                        .as_ref()
                        .map_or(name.clone(), |(_, r)| r.to_string());
                    if bound != "_" {
                        add(scope.aliases.entry(bound).or_default(), vec![name], dropped);
                    }
                }
                _ => {}
            }
        }
        scope
    }
    fn bind(&mut self, prefix: &mut Vec<String>, t: &syn::UseTree, dropped: &Cell<bool>) {
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
                add(self.aliases.entry(bound).or_default(), full, dropped);
            }
        };
        match t {
            syn::UseTree::Path(p) => {
                prefix.push(p.ident.to_string());
                self.bind(prefix, &p.tree, dropped);
                prefix.pop();
            }
            syn::UseTree::Name(n) => leaf(&n.ident, &n.ident),
            syn::UseTree::Rename(r) => leaf(&r.ident, &r.rename),
            syn::UseTree::Glob(_) => add(&mut self.globs, prefix.clone(), dropped),
            syn::UseTree::Group(g) => g.items.iter().for_each(|t| self.bind(prefix, t, dropped)),
        }
    }
}

/// Adds a distinct path up to MAX_EXPANSIONS; `dropped` records a path the bound discarded.
fn add(list: &mut Vec<Vec<String>>, path: Vec<String>, dropped: &Cell<bool>) {
    if list.contains(&path) {
        return;
    }
    if list.len() < MAX_EXPANSIONS {
        list.push(path);
    } else {
        dropped.set(true);
    }
}

/// Collects `use` trees and expression/type paths of one module with aliases expanded;
/// nested modules are walked separately. Each scope stores bindings already expanded in the
/// scope that declared them, so a later inner binding of the same name cannot change them.
/// `dropped` records that a bound discarded an alias expansion, and `unanalyzed` the constructs
/// outside the allow-lists (macros, attributes, modules in blocks); both become gaps.
struct Paths {
    paths: Vec<PathRef>,
    scopes: Vec<Scope>,
    dropped: Cell<bool>,
    /// Expansion steps left for the current binding or mention: cfg branches that bind names
    /// to each other can otherwise branch exponentially before the result bound applies.
    steps: Cell<usize>,
    unanalyzed: BTreeSet<String>,
    plain_macros: BTreeSet<String>,
    defined_macros: BTreeSet<String>,
    imports_macros: bool,
}

/// Built-in attributes that generate no code reaching other modules.
const PLAIN_ATTRIBUTES: [&str; 42] = [
    "allow",
    "automatically_derived",
    "cfg",
    "cold",
    "crate_name",
    "crate_type",
    "default",
    "deny",
    "deprecated",
    "doc",
    "expect",
    "export_name",
    "forbid",
    "global_allocator",
    "ignore",
    "inline",
    "instruction_set",
    "link",
    "link_name",
    "link_section",
    "macro_export",
    "macro_use",
    "must_use",
    "no_builtins",
    "no_implicit_prelude",
    "no_main",
    "no_mangle",
    "no_std",
    "non_exhaustive",
    "panic_handler",
    "path",
    "recursion_limit",
    "repr",
    "should_panic",
    "target_feature",
    "test",
    "track_caller",
    "type_length_limit",
    "unsafe",
    "used",
    "warn",
    "windows_subsystem",
];
/// Standard traits whose derives generate code only against the standard library.
const PLAIN_DERIVES: [&str; 9] = [
    "Clone",
    "Copy",
    "Debug",
    "Default",
    "Eq",
    "Hash",
    "Ord",
    "PartialEq",
    "PartialOrd",
];
/// Tool namespaces whose attributes only configure lints and formatting.
const PLAIN_TOOLS: [&str; 3] = ["clippy", "diagnostic", "rustfmt"];

/// The name of an attribute outside the allow-lists, if any. `derive` is checked per trait and
/// `cfg_attr` per attribute it applies.
fn unanalyzed_attribute(meta: &syn::Meta) -> Option<String> {
    use syn::{Meta, Token, punctuated::Punctuated};
    let path = meta.path();
    let name = path
        .segments
        .last()
        .map(|s| s.ident.to_string())
        .unwrap_or_default();
    if path.segments.len() > 1 {
        let tool = path.segments.first().map(|s| s.ident.to_string());
        return (!tool.is_some_and(|t| PLAIN_TOOLS.contains(&t.as_str()))).then(|| path_text(path));
    }
    match name.as_str() {
        "derive" => {
            let Meta::List(list) = meta else {
                return Some(name);
            };
            let Ok(traits) =
                list.parse_args_with(Punctuated::<syn::Path, Token![,]>::parse_terminated)
            else {
                return Some(name);
            };
            traits
                .iter()
                .find(|t| {
                    let last = t.segments.last().map(|s| s.ident.to_string());
                    let std = t.segments.len() == 1
                        || t.segments
                            .first()
                            .is_some_and(|s| s.ident == "std" || s.ident == "core");
                    !(std && last.is_some_and(|l| PLAIN_DERIVES.contains(&l.as_str())))
                })
                .map(|t| format!("derive({})", path_text(t)))
        }
        "cfg_attr" => {
            let Meta::List(list) = meta else {
                return Some(name);
            };
            let Ok(parts) = list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
            else {
                return Some(name);
            };
            parts.iter().skip(1).find_map(unanalyzed_attribute)
        }
        "unsafe" => {
            let Meta::List(list) = meta else {
                return None;
            };
            list.parse_args::<Meta>()
                .map_or(Some(name), |inner| unanalyzed_attribute(&inner))
        }
        n if PLAIN_ATTRIBUTES.contains(&n) => None,
        _ => Some(name),
    }
}

/// Whether a `cfg_attr` (at any nesting depth) applies a `path` attribute; one that cannot
/// be parsed counts as applying it.
fn mentions_path_attribute(meta: &syn::Meta) -> bool {
    use syn::{Meta, Token, punctuated::Punctuated};
    match meta {
        Meta::NameValue(nv) => nv.path.is_ident("path"),
        Meta::List(list) if list.path.is_ident("cfg_attr") => list
            .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
            .map_or(true, |parts| {
                parts.iter().skip(1).any(mentions_path_attribute)
            }),
        _ => false,
    }
}

/// Single-segment trait names in a `derive` list, also inside `cfg_attr` at any depth.
fn derive_names(meta: &syn::Meta) -> Vec<String> {
    use syn::{Meta, Token, punctuated::Punctuated};
    let Meta::List(list) = meta else {
        return vec![];
    };
    if list.path.is_ident("cfg_attr") {
        return list
            .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
            .map(|parts| parts.iter().skip(1).flat_map(derive_names).collect())
            .unwrap_or_default();
    }
    if !list.path.is_ident("derive") {
        return vec![];
    }
    list.parse_args_with(Punctuated::<syn::Path, Token![,]>::parse_terminated)
        .map(|traits| {
            traits
                .iter()
                .filter(|t| t.segments.len() == 1)
                .map(path_text)
                .collect()
        })
        .unwrap_or_default()
}

fn path_text(p: &syn::Path) -> String {
    p.segments
        .iter()
        .map(|s| s.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}

/// Standard macros that expand only to their arguments plus standard library code.
const PLAIN_MACROS: [&str; 31] = [
    "assert",
    "assert_eq",
    "assert_ne",
    "cfg",
    "column",
    "compile_error",
    "concat",
    "dbg",
    "debug_assert",
    "debug_assert_eq",
    "debug_assert_ne",
    "env",
    "eprint",
    "eprintln",
    "file",
    "format",
    "format_args",
    "line",
    "matches",
    "module_path",
    "option_env",
    "panic",
    "print",
    "println",
    "stringify",
    "todo",
    "unimplemented",
    "unreachable",
    "vec",
    "write",
    "writeln",
];

/// Whether a token stream contains `::` at any depth.
fn has_path(tokens: proc_macro2::TokenStream) -> bool {
    let mut colon = false;
    for t in tokens {
        match t {
            proc_macro2::TokenTree::Punct(p) if p.as_char() == ':' => {
                if colon {
                    return true;
                }
                colon = p.spacing() == proc_macro2::Spacing::Joint;
            }
            proc_macro2::TokenTree::Group(g) => {
                if has_path(g.stream()) {
                    return true;
                }
                colon = false;
            }
            _ => colon = false,
        }
    }
    false
}
impl Paths {
    fn new(items: &[syn::Item]) -> Self {
        let mut p = Paths {
            paths: vec![],
            scopes: vec![],
            dropped: Cell::new(false),
            steps: Cell::new(0),
            unanalyzed: BTreeSet::new(),
            plain_macros: BTreeSet::new(),
            defined_macros: BTreeSet::new(),
            imports_macros: false,
        };
        p.enter(items.iter());
        p
    }
    fn enter<'a>(&mut self, items: impl Iterator<Item = &'a syn::Item>) {
        let raw = Scope::of(items, &self.dropped);
        let mut scope = Scope::default();
        for (name, fulls) in &raw.aliases {
            let list = scope.aliases.entry(name.clone()).or_default();
            for f in fulls {
                let mut out = vec![];
                self.steps.set(MAX_EXPAND_STEPS);
                self.expand(f.clone(), Some(&raw), 0, &mut out);
                out.into_iter().for_each(|e| add(list, e, &self.dropped));
            }
        }
        for g in &raw.globs {
            let mut out = vec![];
            self.steps.set(MAX_EXPAND_STEPS);
            self.expand(g.clone(), Some(&raw), 0, &mut out);
            out.into_iter()
                .for_each(|e| add(&mut scope.globs, e, &self.dropped));
        }
        self.scopes.push(scope);
    }
    /// Expansions of `segs`. Names bound in the declaring scope `raw` are followed (bounded);
    /// otherwise the innermost enclosing binding replaces the first segment once, its paths
    /// being expanded already. A binding whose path starts with its own name (`use fx_core;`)
    /// names the outer item, so expansion stops there.
    fn expand(
        &self,
        segs: Vec<String>,
        raw: Option<&Scope>,
        depth: usize,
        out: &mut Vec<Vec<String>>,
    ) {
        if self.steps.get() == 0 {
            return self.dropped.set(true);
        }
        self.steps.set(self.steps.get() - 1);
        let Some(first) = segs.first() else {
            return add(out, segs, &self.dropped);
        };
        let same_scope = raw.and_then(|r| r.aliases.get(first));
        if same_scope.is_some() && depth >= MAX_ALIAS_DEPTH {
            self.dropped.set(true);
        }
        if let Some(fulls) = same_scope
            && depth < MAX_ALIAS_DEPTH
        {
            for f in fulls {
                if f.first() == Some(first) {
                    add(out, segs.clone(), &self.dropped);
                } else {
                    self.expand([f.as_slice(), &segs[1..]].concat(), raw, depth + 1, out);
                }
            }
            return;
        }
        match self.scopes.iter().rev().find_map(|s| s.aliases.get(first)) {
            Some(fulls) => {
                for f in fulls {
                    if f.first() == Some(first) {
                        add(out, segs.clone(), &self.dropped);
                    } else {
                        add(out, [f.as_slice(), &segs[1..]].concat(), &self.dropped);
                    }
                }
            }
            None => add(out, segs, &self.dropped),
        }
    }
    /// Whether a single-segment name may not be the standard item: a scope binds it, or a glob
    /// import from outside this crate and the standard library could bring it in.
    fn rebound(&self, name: &str) -> bool {
        self.scopes.iter().any(|s| {
            s.aliases.contains_key(name)
                || s.globs.iter().any(|g| {
                    !g.first().is_some_and(|f| {
                        ["crate", "self", "super", "std", "core", "alloc"].contains(&f.as_str())
                    })
                })
        })
    }
    fn push(&mut self, segs: Vec<String>) {
        let mut all = vec![];
        self.steps.set(MAX_EXPAND_STEPS);
        self.expand(segs, None, 0, &mut all);
        let globs: Vec<Vec<String>> = self.scopes.iter().flat_map(|s| s.globs.clone()).collect();
        for segs in all {
            self.paths.push(PathRef {
                segs,
                globs: globs.clone(),
            });
        }
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
    /// Visits the arguments of a standard macro: comma-separated expressions, `vec![x; n]` or
    /// `matches!(x, Pattern if guard)`. False when they do not parse in any of those forms.
    fn plain_arguments(&mut self, m: &syn::Macro, name: &str) -> bool {
        use syn::{Expr, Pat, Token, parse::ParseStream, punctuated::Punctuated};
        if let Ok(args) = m.parse_body_with(Punctuated::<Expr, Token![,]>::parse_terminated) {
            args.iter().for_each(|e| self.visit_expr(e));
            return true;
        }
        if name == "vec"
            && let Ok(args) = m.parse_body_with(Punctuated::<Expr, Token![;]>::parse_terminated)
        {
            args.iter().for_each(|e| self.visit_expr(e));
            return true;
        }
        if name == "matches" {
            let parsed = m.parse_body_with(|input: ParseStream| {
                let value: Expr = input.parse()?;
                input.parse::<Token![,]>()?;
                let pattern = Pat::parse_multi_with_leading_vert(input)?;
                let guard = if input.peek(Token![if]) {
                    input.parse::<Token![if]>()?;
                    Some(input.parse::<Expr>()?)
                } else {
                    None
                };
                if input.peek(Token![,]) {
                    input.parse::<Token![,]>()?;
                }
                Ok((value, pattern, guard))
            });
            if let Ok((value, pattern, guard)) = parsed {
                self.visit_expr(&value);
                self.visit_pat(&pattern);
                guard.iter().for_each(|g| self.visit_expr(g));
                return true;
            }
        }
        false
    }
}
impl<'ast> Visit<'ast> for Paths {
    /// Module-level `mod` items are walked by `Walk::items`; one reached here sits inside a
    /// block or other item, which the walk does not enter, so it is a gap.
    fn visit_item_mod(&mut self, m: &'ast syn::ItemMod) {
        self.unanalyzed
            .insert(format!("mod {} inside a block", m.ident));
    }
    fn visit_item_use(&mut self, u: &'ast syn::ItemUse) {
        u.attrs.iter().for_each(|a| self.visit_attribute(a));
        self.use_tree(&mut vec![], &u.tree);
    }
    /// `extern crate` binds a name (see `Scope::of`) and records its path; with
    /// `#[macro_use]` it also imports macros whose names are not known here.
    fn visit_item_extern_crate(&mut self, e: &'ast syn::ItemExternCrate) {
        visit::visit_item_extern_crate(self, e);
        if e.attrs.iter().any(|a| a.path().is_ident("macro_use")) {
            self.imports_macros = true;
        }
        self.push(vec![e.ident.to_string()]);
    }
    fn visit_item_macro(&mut self, m: &'ast syn::ItemMacro) {
        if m.mac.path.is_ident("macro_rules")
            && let Some(name) = &m.ident
        {
            self.defined_macros.insert(name.to_string());
        }
        visit::visit_item_macro(self, m);
    }
    /// syn keeps tokens it cannot parse as `Verbatim`; they are not analyzed.
    fn visit_item(&mut self, i: &'ast syn::Item) {
        if matches!(i, syn::Item::Verbatim(_)) {
            self.unanalyzed.insert("an unparsed item".into());
        }
        visit::visit_item(self, i);
    }
    fn visit_expr(&mut self, e: &'ast syn::Expr) {
        if matches!(e, syn::Expr::Verbatim(_)) {
            self.unanalyzed.insert("an unparsed expression".into());
        }
        visit::visit_expr(self, e);
    }
    fn visit_type(&mut self, t: &'ast syn::Type) {
        if matches!(t, syn::Type::Verbatim(_)) {
            self.unanalyzed.insert("an unparsed type".into());
        }
        visit::visit_type(self, t);
    }
    fn visit_block(&mut self, b: &'ast syn::Block) {
        self.enter(b.stmts.iter().filter_map(|s| match s {
            syn::Stmt::Item(i) => Some(i),
            _ => None,
        }));
        visit::visit_block(self, b);
        self.scopes.pop();
    }
    /// Allow-list: only standard macros that expand to their arguments plus standard library
    /// code (`println!`, `vec!`, `assert!` ...) and whose arguments parse here are analyzed;
    /// their arguments are visited like code. Every other macro call (`include!`, `json!`, the
    /// crate's own, a standard macro whose arguments do not parse) is recorded as a gap, since
    /// its expansion may reach other modules. A `macro_rules!` definition is a gap when its
    /// body mentions a path.
    fn visit_macro(&mut self, m: &'ast syn::Macro) {
        visit::visit_macro(self, m);
        let name = m
            .path
            .segments
            .last()
            .map(|s| s.ident.to_string())
            .unwrap_or_default();
        if name == "macro_rules" {
            if has_path(m.tokens.clone()) {
                self.unanalyzed.insert(format!("{name}!"));
            }
            return;
        }
        let std_path = m.path.segments.len() == 1
            || m.path
                .segments
                .first()
                .is_some_and(|s| s.ident == "std" || s.ident == "core" || s.ident == "alloc");
        // A `use` binding of the same name (`use other::vec;`) or an external glob import
        // (`use other::*;`) may mean another macro.
        let bound = m.path.segments.len() == 1 && self.rebound(&name);
        let plain = std_path && !bound && PLAIN_MACROS.contains(&name.as_str());
        if plain && m.path.segments.len() == 1 {
            self.plain_macros.insert(name.clone());
        }
        let parsed = plain && self.plain_arguments(m, &name);
        if !parsed {
            self.unanalyzed.insert(format!("{name}!"));
        }
    }
    /// Allow-list for attributes: built-in attributes and derives of standard traits only.
    /// Anything else (`#[serde(...)]`, `#[derive(Serialize)]`, attribute macros) may generate
    /// code that reaches other modules, so it is recorded as a gap.
    fn visit_attribute(&mut self, a: &'ast syn::Attribute) {
        visit::visit_attribute(self, a);
        if let Some(name) = unanalyzed_attribute(&a.meta) {
            self.unanalyzed.insert(format!("#[{name}]"));
        } else {
            // A standard trait name bound by `use` (`use x::Custom as Clone;`) or possibly
            // brought in by an external glob is another derive; one imported by
            // `#[macro_use] extern crate` is checked crate-wide. `cfg_attr` derives count too.
            for name in derive_names(&a.meta) {
                if self.rebound(&name) {
                    self.unanalyzed.insert(format!("#[derive({name})]"));
                } else {
                    self.plain_macros.insert(format!("derive({name})"));
                }
            }
        }
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
    #[test]
    fn aliases_keep_declaration_scope_and_cfg_branches() {
        let got = paths(
            "use crate::a as x; use x as y; fn f() { use crate::b as x; y::sub::f(); } \
             #[cfg(unix)] use crate::u as imp; #[cfg(windows)] use crate::w as imp; fn g() { imp::s::h(); }",
        );
        for want in ["crate::a::sub::f", "crate::u::s::h", "crate::w::s::h"] {
            assert!(got.iter().any(|p| p == want), "{want} not in {got:?}");
        }
        assert!(!got.iter().any(|p| p == "crate::b::sub::f"), "{got:?}");
    }
}
