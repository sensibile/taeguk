mod elixir;
mod go;
mod python;
mod rust;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    env, fs,
    path::{Component, Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

const SCHEMA: &str = "taeguk.code-graph/1";
const LANGS: [&str; 4] = ["elixir", "go", "python", "rust"];

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) struct Node {
    id: String,
    lang: String,
    files: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) struct Edge {
    from: String,
    to: String,
    kind: String,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) struct Gap {
    lang: String,
    kind: String,
    file: Option<String>,
    detail: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Analyzer {
    lang: String,
    tool: String,
    version: Option<String>,
    command: Vec<String>,
    exit_code: Option<i32>,
    status: String,
    error: Option<String>,
}
#[derive(Debug, Default)]
pub(crate) struct Partial {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    gaps: Vec<Gap>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Graph {
    schema: String,
    root: String,
    commit: String,
    dirty: bool,
    dirty_paths: Vec<String>,
    generated_at: u64,
    status: String,
    analyzers: Vec<Analyzer>,
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    gaps: Vec<Gap>,
    digest: String,
}

impl Node {
    fn new(lang: &str, id: impl Into<String>, files: Vec<String>) -> Self {
        Node {
            id: format!("{lang}:{}", id.into()),
            lang: lang.into(),
            files,
        }
    }
}
impl Edge {
    fn new(from: &str, to: &str, kind: &str) -> Self {
        Edge {
            from: from.into(),
            to: to.into(),
            kind: kind.into(),
        }
    }
}
impl Gap {
    fn new(lang: &str, kind: &str, file: Option<&str>, detail: impl Into<String>) -> Self {
        Gap {
            lang: lang.into(),
            kind: kind.into(),
            file: file.map(Into::into),
            detail: detail.into(),
        }
    }
}
impl Analyzer {
    fn new(lang: &str, tool: &str, command: &[&str]) -> Self {
        Analyzer {
            lang: lang.into(),
            tool: tool.into(),
            version: None,
            command: command.iter().map(|s| s.to_string()).collect(),
            exit_code: None,
            status: "FAIL".into(),
            error: None,
        }
    }
}

pub(crate) struct Output {
    pub(crate) code: Option<i32>,
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: String,
}
pub(crate) fn exec(cmd: &mut Command) -> Result<Output, String> {
    let o = cmd
        .output()
        .map_err(|e| format!("cannot run {:?}: {e}", cmd.get_program()))?;
    Ok(Output {
        code: o.status.code(),
        stdout: o.stdout,
        stderr: String::from_utf8_lossy(&o.stderr).into_owned(),
    })
}
/// First non-empty line of `<tool> <arg>`; tools print versions on stdout or stderr.
pub(crate) fn version(tool: &str, args: &[&str]) -> Option<String> {
    let o = exec(Command::new(tool).args(args)).ok()?;
    let text = String::from_utf8_lossy(&o.stdout).into_owned() + &o.stderr;
    text.lines()
        .map(str::trim)
        .rfind(|l| !l.is_empty())
        .map(Into::into)
}
/// Version of the tool an analyzer runs; impact compares it with the version in the graph.
pub(crate) fn tool_version(lang: &str) -> Option<String> {
    match lang {
        "elixir" => version("mix", &["--version"]),
        "go" => version("go", &["version"]),
        "python" => version("python3", &["--version"]),
        "rust" => version("cargo", &["--version"])
            .map(|c| format!("{c}; taeguk-cli {} syn 3", env!("CARGO_PKG_VERSION"))),
        _ => None,
    }
}
fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let o = exec(Command::new("git").arg("-C").arg(root).args(args))?;
    if o.code != Some(0) {
        return Err(format!(
            "git {} failed: {}",
            args.join(" "),
            o.stderr.trim()
        ));
    }
    Ok(o.stdout)
}
fn nul_list(data: Vec<u8>) -> Result<Vec<String>, String> {
    let text = String::from_utf8(data).map_err(|_| "git returned a non-UTF-8 path")?;
    Ok(text
        .split('\0')
        .filter(|s| !s.is_empty())
        .map(Into::into)
        .collect())
}
fn head(root: &Path) -> Result<String, String> {
    Ok(String::from_utf8_lossy(&git(root, &["rev-parse", "HEAD"])?)
        .trim()
        .into())
}
/// Tracked files under ROOT, relative to ROOT.
pub(crate) fn tracked(root: &Path) -> Result<Vec<String>, String> {
    nul_list(git(root, &["ls-files", "-z"])?)
}
fn changed_since(root: &Path, base: &str) -> Result<Vec<String>, String> {
    nul_list(git(
        root,
        &["diff", "--name-only", "--relative", "-z", base, "--"],
    )?)
}

/// Relative path from `base` to `path` when `path` is inside `base`.
pub(crate) fn relative(base: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(base).ok()?;
    let parts: Vec<_> = rel
        .components()
        .map(|c| c.as_os_str().to_str())
        .collect::<Option<_>>()?;
    Some(parts.join("/"))
}
/// Lexically normalize `a/b/../c` without touching the file system.
pub(crate) fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            c => out.push(c),
        }
    }
    out
}

pub(crate) struct TempDir(pub(crate) PathBuf);
impl TempDir {
    pub(crate) fn new(tag: &str) -> Result<Self, String> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir =
            env::temp_dir().join(format!("taeguk-graph-{tag}-{}-{nanos}", std::process::id()));
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        Ok(TempDir(dir))
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn detect(root: &Path, files: &[String]) -> Vec<String> {
    let mut langs = Vec::new();
    for (lang, marker) in [
        ("elixir", "mix.exs"),
        ("go", "go.mod"),
        ("rust", "Cargo.toml"),
    ] {
        if root.join(marker).is_file() {
            langs.push(lang.to_string());
        }
    }
    if files.iter().any(|f| f.ends_with(".py")) {
        langs.push("python".into());
    }
    langs.sort();
    langs
}

fn digest(g: &Graph) -> String {
    let body = json!({"schema": g.schema, "commit": g.commit, "dirty": g.dirty,
        "dirty_paths": g.dirty_paths, "status": g.status, "analyzers": g.analyzers,
        "nodes": g.nodes, "edges": g.edges, "gaps": g.gaps});
    format!("sha256:{:x}", Sha256::digest(body.to_string().as_bytes()))
}

fn build(root: &Path, langs: &[String], out: &Path) -> Result<Value, String> {
    let root = root
        .canonicalize()
        .map_err(|e| format!("{}: {e}", root.display()))?;
    let commit = head(&root)?;
    let files = tracked(&root)?;
    let dirty_paths = nul_list(git(
        &root,
        &["diff", "HEAD", "--name-only", "--relative", "-z", "--"],
    )?)?;
    let langs = if langs.is_empty() {
        detect(&root, &files)
    } else {
        langs.to_vec()
    };
    if langs.is_empty() {
        return Err("no supported language detected; pass --lang".into());
    }
    let (mut analyzers, mut nodes, mut edges, mut gaps) = (vec![], vec![], vec![], vec![]);
    for lang in &langs {
        let (a, result) = match lang.as_str() {
            "elixir" => elixir::analyze(&root, &files),
            "go" => go::analyze(&root),
            "python" => python::analyze(&root, &files),
            "rust" => rust::analyze(&root),
            other => return Err(format!("unsupported language: {other}")),
        };
        let mut a = a;
        match result {
            Ok(p) => {
                a.status = "PASS".into();
                nodes.extend(p.nodes);
                edges.extend(p.edges);
                gaps.extend(p.gaps);
            }
            // A failed analyzer contributes nothing so a partial graph is never presented as complete.
            Err(e) => {
                a.status = "FAIL".into();
                a.error = Some(e);
            }
        }
        analyzers.push(a);
    }
    let known: BTreeSet<_> = nodes.iter().map(|n| n.id.clone()).collect();
    edges.retain(|e| e.from != e.to && known.contains(&e.from) && known.contains(&e.to));
    nodes.sort();
    nodes.dedup_by(|a, b| a.id == b.id);
    edges.sort();
    edges.dedup();
    gaps.sort();
    gaps.dedup();
    let status = if analyzers.iter().all(|a| a.status == "PASS") {
        "PASS"
    } else {
        "FAIL"
    };
    let mut g = Graph {
        schema: SCHEMA.into(),
        root: root.display().to_string(),
        commit,
        dirty: !dirty_paths.is_empty(),
        dirty_paths,
        generated_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        status: status.into(),
        analyzers,
        nodes,
        edges,
        gaps,
        digest: String::new(),
    };
    g.digest = digest(&g);
    let text = serde_json::to_string_pretty(&g).map_err(|e| e.to_string())?;
    fs::write(out, text + "\n").map_err(|e| format!("{}: {e}", out.display()))?;
    Ok(
        json!({"status": g.status, "out": out.display().to_string(), "digest": g.digest,
        "commit": g.commit, "dirty": g.dirty, "nodes": g.nodes.len(), "edges": g.edges.len(),
        "gaps": g.gaps.len(), "analyzers": g.analyzers}),
    )
}

fn read_graph(path: &Path) -> Result<Graph, String> {
    let data = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let g: Graph = serde_json::from_slice(&data).map_err(|e| e.to_string())?;
    if g.schema != SCHEMA {
        return Err(format!("unsupported graph schema: {}", g.schema));
    }
    if digest(&g) != g.digest {
        return Err("graph digest does not match its content".into());
    }
    Ok(g)
}

fn impact(g: &Graph, root: &Path, head: &str, changed: &[String], depth: Option<usize>) -> Value {
    let mut reasons = Vec::new();
    // Paths are ROOT-relative, so a graph of another directory must not be reused.
    if root.display().to_string() != g.root {
        reasons.push("graph_root_differs");
    }
    if g.analyzers
        .iter()
        .any(|a| a.version.is_none() || tool_version(&a.lang) != a.version)
    {
        reasons.push("analyzer_version_changed_or_unknown");
    }
    if g.status != "PASS" {
        reasons.push("graph_status_not_pass");
    }
    if g.commit != head {
        reasons.push("graph_commit_differs_from_head");
    }
    if g.dirty {
        reasons.push("graph_built_from_dirty_tree");
    }
    let mut owner: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for n in &g.nodes {
        for f in &n.files {
            owner.entry(f.as_str()).or_default().push(n.id.as_str());
        }
    }
    let mut reverse: BTreeMap<&str, Vec<&Edge>> = BTreeMap::new();
    for e in &g.edges {
        reverse.entry(e.to.as_str()).or_default().push(e);
    }
    let mut dist: BTreeMap<&str, usize> = BTreeMap::new();
    let mut queue = VecDeque::new();
    let mut unmapped = Vec::new();
    for f in changed {
        match owner.get(f.as_str()) {
            Some(ids) => {
                for id in ids {
                    if dist.insert(id, 0).is_none() {
                        queue.push_back(*id);
                    }
                }
            }
            None => unmapped.push(f.clone()),
        }
    }
    if !unmapped.is_empty() {
        reasons.push("changed_files_not_in_graph");
    }
    while let Some(id) = queue.pop_front() {
        let d = dist[id];
        if depth.is_some_and(|max| d >= max) {
            continue;
        }
        for e in reverse.get(id).into_iter().flatten() {
            if !dist.contains_key(e.from.as_str()) {
                dist.insert(&e.from, d + 1);
                queue.push_back(&e.from);
            }
        }
    }
    let by_id: BTreeMap<_, _> = g.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    let mut impacted: Vec<_> = dist.iter().map(|(id, d)| (*d, *id)).collect();
    impacted.sort();
    let impacted: Vec<Value> = impacted
        .into_iter()
        .map(|(d, id)| json!({"id": id, "distance": d, "files": by_id[id].files}))
        .collect();
    let status = if reasons.is_empty() {
        "PASS"
    } else {
        "UNKNOWN"
    };
    json!({"status": status, "reasons": reasons, "graph_digest": g.digest, "graph_commit": g.commit,
        "head": head, "changed": changed, "unmapped": unmapped, "depth_limit": depth,
        "impacted": impacted, "gaps": g.gaps,
        "limits": ["static source dependencies only",
            "runtime, configuration, data and dynamic dispatch paths are not included",
            "a missing edge is not evidence of no impact",
            "PASS means the graph was current and every changed file was mapped, not that unlisted code is unaffected"]})
}

fn usage() -> String {
    "usage: taeguk graph build ROOT --out GRAPH.json [--lang elixir|go|python|rust]... | taeguk graph impact GRAPH.json ROOT (--changed PATH... | --git-diff BASE) [--depth N]".into()
}

pub(crate) fn run(args: &[String]) -> Result<Value, String> {
    match args {
        [cmd, root, rest @ ..] if cmd == "build" => {
            let (mut out, mut langs) = (None, vec![]);
            let mut it = rest.iter();
            while let Some(flag) = it.next() {
                let value = it.next().ok_or_else(usage)?;
                match flag.as_str() {
                    "--out" => out = Some(PathBuf::from(value)),
                    "--lang" if LANGS.contains(&value.as_str()) => langs.push(value.clone()),
                    _ => return Err(usage()),
                }
            }
            langs.sort();
            langs.dedup();
            build(Path::new(root), &langs, &out.ok_or_else(usage)?)
        }
        [cmd, graph, root, rest @ ..] if cmd == "impact" => {
            let g = read_graph(Path::new(graph))?;
            let root = Path::new(root)
                .canonicalize()
                .map_err(|e| format!("{root}: {e}"))?;
            let root = root.as_path();
            let (mut changed, mut base, mut depth) = (vec![], None, None);
            let mut it = rest.iter().peekable();
            while let Some(flag) = it.next() {
                match flag.as_str() {
                    "--changed" => {
                        while let Some(p) = it.next_if(|a| !a.starts_with("--")) {
                            let norm = normalize(Path::new(p));
                            if norm.is_absolute() || p.split('/').any(|c| c == "..") {
                                return Err(format!("changed path must be relative to ROOT: {p}"));
                            }
                            changed.push(norm.to_string_lossy().into_owned());
                        }
                    }
                    "--git-diff" => base = Some(it.next().ok_or_else(usage)?.clone()),
                    "--depth" => {
                        depth = Some(it.next().and_then(|d| d.parse().ok()).ok_or_else(usage)?)
                    }
                    _ => return Err(usage()),
                }
            }
            match (changed.is_empty(), base) {
                (false, None) => {}
                (true, Some(b)) => changed = changed_since(root, &b)?,
                _ => return Err(usage()),
            }
            changed.sort();
            changed.dedup();
            Ok(impact(&g, root, &head(root)?, &changed, depth))
        }
        _ => Err(usage()),
    }
}
