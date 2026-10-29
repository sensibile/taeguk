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
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
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
#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct Partial {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    gaps: Vec<Gap>,
    /// ROOT-relative files the analyzer read beyond the language's source patterns (a Mix
    /// manifest, a `#[path]` file with another extension); they join the input fingerprint.
    #[serde(default)]
    read: Vec<String>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Graph {
    schema: String,
    root: String,
    commit: String,
    dirty: bool,
    dirty_paths: Vec<String>,
    /// Files analyzers reported reading beyond the source patterns, and the fingerprint of all
    /// analyzer inputs.
    read: Vec<String>,
    inputs: String,
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
/// Runs a command with `input` on stdin. Data derived from the target repository (file names,
/// paths inside ROOT) goes to tools this way, never as command-line arguments, where a name like
/// `-r` would be read as an option.
pub(crate) fn exec_input(cmd: &mut Command, input: &[u8]) -> Result<Output, String> {
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot run {:?}: {e}", cmd.get_program()))?;
    let mut stdin = child.stdin.take().ok_or("stdin unavailable")?;
    let input = input.to_vec();
    let writer = std::thread::spawn(move || stdin.write_all(&input));
    let o = child.wait_with_output().map_err(|e| e.to_string())?;
    let _ = writer.join();
    Ok(Output {
        code: o.status.code(),
        stdout: o.stdout,
        stderr: String::from_utf8_lossy(&o.stderr).into_owned(),
    })
}
/// Last non-empty line of `<tool> <arg>`. Like the analyzers it runs outside ROOT, so the
/// user's environment picks the toolchain, never a toolchain file in the target repository.
pub(crate) fn version(tool: &str, args: &[&str]) -> Option<String> {
    let dir = TempDir::new("version").ok()?;
    let o = exec(
        Command::new(tool)
            .args(args)
            .current_dir(&dir.0)
            .env("RUSTUP_AUTO_INSTALL", "0")
            .env("GOTOOLCHAIN", "local"),
    )
    .ok()?;
    let text = String::from_utf8_lossy(&o.stdout).into_owned() + &o.stderr;
    text.lines()
        .map(str::trim)
        .rfind(|l| !l.is_empty())
        .map(Into::into)
}
/// Version of the tool an analyzer runs; impact compares it with the version in the graph.
pub(crate) fn tool_version(lang: &str) -> Option<String> {
    match lang {
        "elixir" => version("elixir", &["--version"]),
        "go" => version("go", &["version"]),
        "python" => version("python3", &["--version"]),
        "rust" => version("cargo", &["--version"])
            .map(|c| format!("{c}; taeguk-cli {} syn 3", env!("CARGO_PKG_VERSION"))),
        _ => None,
    }
}
/// The empty tree, used as the attribute source so `.gitattributes` in the tree adds nothing.
const EMPTY_TREE: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

/// git in ROOT. Only commands that never pass working tree files through filter drivers are
/// used (see `dirty_paths`): attributes can also come from `core.attributesFile` or
/// `.git/info/attributes`, which no option disables, so the safe boundary is not reading file
/// contents through git at all. The fsmonitor hook is off as well.
fn git_command(root: &Path, args: &[&str]) -> Command {
    let mut cmd = Command::new("git");
    cmd.args(["-c", "core.fsmonitor=false"])
        .arg(format!("--attr-source={EMPTY_TREE}"))
        .arg("-C")
        .arg(root)
        .args(args);
    cmd
}
fn git_done(args: &[&str], o: Output) -> Result<Vec<u8>, String> {
    if o.code != Some(0) {
        return Err(format!(
            "git {} failed: {}",
            args.join(" "),
            o.stderr.trim()
        ));
    }
    Ok(o.stdout)
}
fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    git_done(args, exec(&mut git_command(root, args))?)
}
fn git_input(root: &Path, args: &[&str], input: &[u8]) -> Result<Vec<u8>, String> {
    git_done(args, exec_input(&mut git_command(root, args), input)?)
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
/// Paths that differ from HEAD in the index or working tree, plus untracked files that are
/// not ignored: analyzers such as `go list` read those too. `git diff` against the working
/// tree would run clean filters configured in the checkout, so the index is compared with
/// HEAD by git (no working tree file read) and working tree files with the index here: regular
/// files are hashed with `git hash-object --no-filters`, links compared by target, and anything
/// else (a FIFO) counts as changed without being opened. A repository that uses filters (LFS)
/// therefore always looks changed, which only makes impact UNKNOWN.
fn dirty_paths(root: &Path) -> Result<Vec<String>, String> {
    let mut paths = nul_list(git(
        root,
        &[
            "diff",
            "--cached",
            "HEAD",
            "--name-only",
            "--relative",
            "-z",
            "--",
        ],
    )?)?;
    paths.extend(worktree_changes(root)?);
    paths.extend(untracked(root)?);
    paths.sort();
    paths.dedup();
    Ok(paths)
}
/// Index entries whose working tree file differs, compared without filters.
fn worktree_changes(root: &Path) -> Result<Vec<String>, String> {
    if root.to_string_lossy().contains('\n') {
        return Err("ROOT path contains a newline".into());
    }
    let (mut changed, mut regular, mut links) = (vec![], vec![], vec![]);
    for entry in nul_list(git(root, &["ls-files", "-s", "-z"])?)? {
        let Some((meta, path)) = entry.split_once('\t') else {
            continue;
        };
        let [mode, sha, stage] = meta.split(' ').collect::<Vec<_>>()[..] else {
            continue;
        };
        if mode == "160000" {
            continue; // submodule
        }
        let kind = fs::symlink_metadata(root.join(path)).map(|m| m.file_type());
        match (mode, kind) {
            _ if stage != "0" => changed.push(path.to_string()),
            ("120000", Ok(k)) if k.is_symlink() => links.push((path.to_string(), sha.to_string())),
            (m, Ok(k)) if m != "120000" && k.is_file() && !path.contains('\n') => {
                regular.push((path.to_string(), sha.to_string()))
            }
            _ => changed.push(path.to_string()),
        }
    }
    if !regular.is_empty() {
        // Absolute paths: git reads --stdin-paths relative to the repository top, not ROOT.
        let input: String = regular
            .iter()
            .map(|(p, _)| format!("{}\n", root.join(p).display()))
            .collect();
        let out = git_input(
            root,
            &["hash-object", "--no-filters", "--stdin-paths"],
            input.as_bytes(),
        )?;
        let hashes: Vec<&str> = std::str::from_utf8(&out)
            .map_err(|_| "git returned a non-UTF-8 hash")?
            .lines()
            .collect();
        if hashes.len() != regular.len() {
            return Err("git hash-object returned an unexpected number of hashes".into());
        }
        for ((path, sha), hash) in regular.into_iter().zip(hashes) {
            if sha != hash {
                changed.push(path);
            }
        }
    }
    if !links.is_empty() {
        let input: String = links.iter().map(|(_, sha)| format!("{sha}\n")).collect();
        let out = git_input(root, &["cat-file", "--batch"], input.as_bytes())?;
        let mut rest = &out[..];
        for (path, _) in links {
            let header_end = rest
                .iter()
                .position(|b| *b == b'\n')
                .ok_or("git cat-file output ended early")?;
            let header = String::from_utf8_lossy(&rest[..header_end]).into_owned();
            let size: usize = header
                .rsplit(' ')
                .next()
                .and_then(|n| n.parse().ok())
                .ok_or("unexpected git cat-file header")?;
            let start = header_end + 1;
            let content = rest
                .get(start..start + size)
                .ok_or("git cat-file output ended early")?;
            rest = rest.get(start + size + 1..).unwrap_or(&[]);
            let target = fs::read_link(root.join(&path)).map_err(|e| format!("{path}: {e}"))?;
            if target.as_os_str().as_encoded_bytes() != content {
                changed.push(path);
            }
        }
    }
    Ok(changed)
}

fn untracked(root: &Path) -> Result<Vec<String>, String> {
    nul_list(git(
        root,
        &["ls-files", "-z", "--others", "--exclude-standard"],
    )?)
}

/// An ignored directory of generated build output that no selected analyzer reads. It is
/// recognized by what the tools write, not by name: a Mix build (`_build/<env>/lib`, which go
/// skips as `_`-prefixed and which is checked while it is copied) or, when go is not selected, a
/// cargo or other cache directory (`.rustc_info.json`, `CACHEDIR.TAG`). A directory merely named
/// `target` can hold workspace members cargo reads, so it is walked.
fn build_output(dir: &Path, name: &std::ffi::OsStr, go: bool) -> bool {
    let mix = name == "_build"
        && fs::read_dir(dir)
            .is_ok_and(|mut d| d.any(|e| e.is_ok_and(|e| e.path().join("lib").is_dir())));
    let cache = dir.join(".rustc_info.json").is_file() || dir.join("CACHEDIR.TAG").is_file();
    mix || (!go && cache)
}

/// Walks ROOT on disk (git does not list special files), calling `visit(rel, path, kind,
/// ignored)` for each entry; a directory is entered when `visit` returns true. `.git` is
/// skipped, and so are ignored directories no selected analyzer reads: `_build` (go skips
/// `_`-prefixed directories, cargo and our Python script do not read it, and the Elixir build is
/// checked while it is copied) and `target` unless go is selected (`go list ./...` reads it).
fn walk(
    root: &Path,
    langs: &[String],
    mut visit: impl FnMut(&str, &Path, fs::FileType, bool) -> Result<bool, String>,
) -> Result<(), String> {
    let ignored: BTreeSet<String> = nul_list(git(
        root,
        &[
            "ls-files",
            "-z",
            "--others",
            "--ignored",
            "--exclude-standard",
            "--directory",
        ],
    )?)?
    .into_iter()
    .map(|p| p.trim_end_matches('/').to_string())
    .collect();
    let go = langs.iter().any(|l| l == "go");
    let mut stack = vec![(root.to_path_buf(), false)];
    while let Some((dir, in_ignored)) = stack.pop() {
        let mut entries: Vec<_> = fs::read_dir(&dir)
            .map_err(|e| format!("{}: {e}", dir.display()))?
            .collect::<Result<_, _>>()
            .map_err(|e| e.to_string())?;
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let path = entry.path();
            let rel = relative(root, &path).ok_or("non-UTF-8 path under ROOT")?;
            let name = entry.file_name();
            let is_ignored = in_ignored || ignored.contains(&rel);
            if rel == ".git" || (is_ignored && build_output(&path, &name, go)) {
                continue;
            }
            let kind = entry.file_type().map_err(|e| format!("{rel}: {e}"))?;
            if visit(&rel, &path, kind, is_ignored)? {
                stack.push((path, is_ignored));
            }
        }
    }
    Ok(())
}

/// Paths under ROOT that external analyzers (go, cargo, mix) may read without our listing them
/// must not block them or exhaust memory. Tracked and untracked (not ignored) paths must be
/// regular files, directories, or links to those inside ROOT. Ignored paths are read too
/// (`go list` does not follow .gitignore) but hold local environments such as `.venv` whose file
/// links leave ROOT, so there special files (FIFO, device, socket), links to them, and links to
/// directories outside ROOT that go would enter are refused.
/// Tracked paths are checked even inside skipped directories.
fn scan_root(root: &Path, tracked: &[String], langs: &[String]) -> Result<(), String> {
    let go = langs.iter().any(|l| l == "go");
    let check =
        |rel: &str, path: &Path, kind: fs::FileType, strict: bool| -> Result<bool, String> {
            if kind.is_dir() {
                return Ok(true);
            }
            if kind.is_symlink() {
                let real = match path.canonicalize() {
                    Ok(real) => real,
                    // A dangling link in an ignored environment cannot be opened by anyone.
                    Err(_) if !strict => return Ok(false),
                    Err(e) => return Err(format!("{rel}: {e}")),
                };
                // A linked directory outside ROOT is never walked here, yet `go list ./...`
                // enters it where go traverses (no `.`/`_`-prefixed or `testdata` component),
                // so there it is refused even when ignored. Anything else an analyzer reads
                // through such a link is bounded by the analyzer time limit and rejected by
                // the analyzers' own inside-ROOT checks on what they report.
                let go_walks = go
                    && !rel
                        .split('/')
                        .any(|c| c.starts_with('.') || c.starts_with('_') || c == "testdata");
                if (strict || (go_walks && real.is_dir())) && !real.starts_with(root) {
                    return Err(format!("{rel} resolves outside ROOT"));
                }
                if !(real.is_file() || real.is_dir()) {
                    return Err(format!("{rel} is not a regular file"));
                }
            } else if !kind.is_file() {
                return Err(format!("{rel} is not a regular file"));
            }
            Ok(false)
        };
    for f in tracked {
        if let Ok(meta) = fs::symlink_metadata(root.join(f)) {
            check(f, &root.join(f), meta.file_type(), true)?;
        }
    }
    walk(root, langs, |rel, path, kind, ignored| {
        check(rel, path, kind, !ignored)
    })
}

/// Whether a file is an input some selected analyzer reads. Go reads every source and module
/// file it finds (ignored ones too), cargo and syn read manifests and `.rs` files, mix the
/// project's Elixir sources; our Python script reads only tracked `.py` files and pyproject.
fn is_input(rel: &str, ignored: bool, langs: &[String]) -> bool {
    let name = rel.rsplit('/').next().unwrap_or(rel);
    let ext = name.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
    let under = |dir: &str| rel.split('/').any(|c| c == dir);
    langs.iter().any(|l| match l.as_str() {
        "go" => {
            ["go", "s", "c", "h"].contains(&ext)
                || ["go.mod", "go.sum", "go.work", "go.work.sum"].contains(&name)
        }
        // Build output is already skipped by the walk; a directory merely named `target` can
        // hold workspace members.
        "rust" => ext == "rs" || name == "Cargo.toml" || name == "Cargo.lock",
        "elixir" => !under("deps") && (ext == "ex" || ext == "exs" || name == "mix.lock"),
        "python" => !ignored && (ext == "py" || name == "pyproject.toml"),
        _ => false,
    })
}

/// Hash of every analyzer input under ROOT: files matching the language source patterns
/// (untracked and ignored ones included) and the files analyzers reported reading (`read`), so
/// impact can tell when anything the graph was built from has changed since. Files over
/// MAX_SOURCE are hashed by size and modification time.
fn input_fingerprint(root: &Path, langs: &[String], read: &[String]) -> Result<String, String> {
    let mut files: BTreeSet<String> = read.iter().cloned().collect();
    walk(root, langs, |rel, _, kind, ignored| {
        if kind.is_dir() {
            return Ok(true);
        }
        if is_input(rel, ignored, langs) {
            files.insert(rel.to_string());
        }
        Ok(false)
    })?;
    let mut h = Sha256::new();
    for rel in &files {
        h.update(rel.as_bytes());
        h.update([0]);
        // Links are followed; the ROOT scan already limited where they may point.
        match fs::metadata(root.join(rel)) {
            Ok(meta) if meta.is_file() && meta.len() > MAX_SOURCE => {
                let mtime = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_nanos())
                    .unwrap_or(0);
                h.update(format!("size={} mtime={mtime}", meta.len()).as_bytes());
            }
            Ok(meta) if meta.is_file() => h.update(Sha256::digest(
                fs::read(root.join(rel)).map_err(|e| format!("{rel}: {e}"))?,
            )),
            _ => h.update(b"absent"),
        }
        h.update([0]);
    }
    Ok(format!("sha256:{:x}", h.finalize()))
}

/// Writes a new file next to `out` and renames it over `out`: the rename replaces whatever `out`
/// is (a link, a FIFO) instead of following or opening it, so a link swapped in after the checks
/// in `output_path` is not written through.
fn write_replacing(out: &Path, text: &str) -> Result<(), String> {
    let dir = out.parent().unwrap_or(Path::new("."));
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let name = out
        .file_name()
        .map(|n| n.to_string_lossy())
        .unwrap_or_default();
    let tmp = dir.join(format!(".{name}.{}.{nanos}.tmp", std::process::id()));
    let written = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)
        .and_then(|mut f| f.write_all(text.as_bytes()))
        .and_then(|()| fs::rename(&tmp, out));
    if let Err(e) = written {
        let _ = fs::remove_file(&tmp);
        return Err(format!("{}: {e}", out.display()));
    }
    Ok(())
}

/// Resolves `--out` so that writing it cannot follow a link: an existing link is refused, and
/// a path written inside ROOT must not leave ROOT through a linked directory.
fn output_path(root: &Path, out: &Path) -> Result<PathBuf, String> {
    let name = out
        .file_name()
        .ok_or_else(|| format!("{}: not a file path", out.display()))?;
    let parent = match out.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => PathBuf::from("."),
    };
    let real = parent
        .canonicalize()
        .map_err(|e| format!("{}: {e}", parent.display()))?
        .join(name);
    if fs::symlink_metadata(&real).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(format!(
            "{} is a link; refusing to write through it",
            out.display()
        ));
    }
    let lexical = normalize(&env::current_dir().map_err(|e| e.to_string())?.join(out));
    let lexical_root = lexical.ancestors().find_map(|a| {
        a.canonicalize()
            .ok()
            .filter(|c| c == root)
            .map(|_| a.to_path_buf())
    });
    if lexical_root.is_some() && !real.starts_with(root) {
        return Err(format!("{} leaves ROOT through a link", out.display()));
    }
    Ok(real)
}
/// Files changed from BASE to the working tree: commits BASE..HEAD (tree comparison, no working
/// tree file read) plus the working tree changes `dirty_paths` finds without filters.
fn changed_since(root: &Path, base: &str) -> Result<Vec<String>, String> {
    let mut paths = nul_list(git(
        root,
        &[
            "diff",
            "--name-only",
            "--relative",
            "-z",
            "--end-of-options",
            base,
            "HEAD",
            "--",
        ],
    )?)?;
    paths.extend(dirty_paths(root)?);
    paths.sort();
    paths.dedup();
    Ok(paths)
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

/// Checks without opening (a FIFO would block on open) that `path` resolves to a regular file
/// inside ROOT (canonical) within MAX_SOURCE, and returns the resolved path.
pub(crate) fn check_source(root: &Path, path: &Path) -> Result<PathBuf, String> {
    let shown = || relative(root, path).unwrap_or_else(|| path.display().to_string());
    let real = path
        .canonicalize()
        .map_err(|e| format!("{}: {e}", shown()))?;
    if !real.starts_with(root) {
        return Err(format!("{} resolves outside ROOT", shown()));
    }
    let meta = fs::metadata(&real).map_err(|e| format!("{}: {e}", shown()))?;
    if !meta.is_file() {
        return Err(format!("{} is not a regular file", shown()));
    }
    if meta.len() > MAX_SOURCE {
        return Err(format!("{} is larger than {MAX_SOURCE} bytes", shown()));
    }
    Ok(real)
}
/// Largest source file an analyzer reads.
const MAX_SOURCE: u64 = 8 << 20;
/// Reads a source file that resolves to a regular file inside ROOT (canonical), within MAX_SOURCE.
/// Returns its canonical path with the text, so links cannot pull in files outside ROOT or devices.
pub(crate) fn read_source(root: &Path, path: &Path) -> Result<(PathBuf, String), String> {
    let shown = || relative(root, path).unwrap_or_else(|| path.display().to_string());
    let real = check_source(root, path)?;
    let mut text = String::new();
    fs::File::open(&real)
        .and_then(|f| f.take(MAX_SOURCE + 1).read_to_string(&mut text))
        .map_err(|e| format!("{}: {e}", shown()))?;
    if text.len() as u64 > MAX_SOURCE {
        return Err(format!("{} is larger than {MAX_SOURCE} bytes", shown()));
    }
    Ok((real, text))
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

/// Runs one analyzer in this process (`graph __analyze LANG ROOT`, used by `isolated`).
fn analyze(root: &Path, lang: &str) -> Result<(Analyzer, Result<Partial, String>), String> {
    let files = tracked(root)?;
    Ok(match lang {
        "elixir" => elixir::analyze(root, &files),
        "go" => go::analyze(root),
        "python" => python::analyze(root, &files),
        "rust" => rust::analyze(root),
        other => return Err(format!("unsupported language: {other}")),
    })
}

/// Default bound on one analyzer process, overridable with TAEGUK_GRAPH_TIMEOUT_SECS.
const ANALYZER_TIMEOUT_SECS: u64 = 600;

/// Runs one analyzer as a child process of this binary, in its own process group, with a time
/// bound. Whatever the input does to a parser or external tool (a crash such as stack
/// exhaustion, or a wait that never ends), the result is that analyzer's FAIL, not a crashed or
/// stuck CLI. The group is killed on timeout so tools it started (cargo, go, mix) end too.
fn isolated(root: &Path, lang: &str) -> (Analyzer, Result<Partial, String>) {
    let failed = |e: String| (Analyzer::new(lang, "-", &[]), Err(e));
    let exe = match env::current_exe() {
        Ok(exe) => exe,
        Err(e) => return failed(format!("cannot locate taeguk executable: {e}")),
    };
    let limit = env::var("TAEGUK_GRAPH_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(ANALYZER_TIMEOUT_SECS);
    let mut cmd = Command::new(exe);
    cmd.args(["graph", "__analyze", lang])
        .arg(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut cmd, 0);
    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(e) => return failed(format!("cannot start analyzer process: {e}")),
    };
    let read = |mut r: Box<dyn Read + Send>| {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = r.read_to_end(&mut buf);
            buf
        })
    };
    let out = read(Box::new(child.stdout.take().expect("piped stdout")));
    let err = read(Box::new(child.stderr.take().expect("piped stderr")));
    let started = std::time::Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if started.elapsed().as_secs() >= limit => {
                #[cfg(unix)]
                let _ = Command::new("kill")
                    .args(["-KILL", &format!("-{}", child.id())])
                    .status();
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(20)),
            Err(e) => return failed(format!("analyzer process: {e}")),
        }
    };
    let (out, err) = (
        out.join().unwrap_or_default(),
        err.join().unwrap_or_default(),
    );
    let Some(status) = status else {
        return failed(format!(
            "analyzer did not finish within {limit} s and was stopped"
        ));
    };
    let reply: Option<Value> = serde_json::from_slice(&out).ok();
    let parsed = reply.and_then(|v| {
        let a: Analyzer = serde_json::from_value(v.get("analyzer")?.clone()).ok()?;
        let result = match (v.get("partial"), v.get("error")) {
            (Some(p), _) if !p.is_null() => Ok(serde_json::from_value(p.clone()).ok()?),
            (_, Some(Value::String(e))) => Err(e.clone()),
            _ => return None,
        };
        Some((a, result))
    });
    match parsed {
        Some(r) if status.success() => r,
        _ => {
            let tail: String = String::from_utf8_lossy(&err)
                .lines()
                .rev()
                .take(3)
                .collect::<Vec<_>>()
                .join(" | ");
            failed(format!(
                "analyzer process exited abnormally ({status}): {tail}"
            ))
        }
    }
}

fn digest(g: &Graph) -> String {
    let body = json!({"schema": g.schema, "commit": g.commit, "dirty": g.dirty,
        "dirty_paths": g.dirty_paths, "read": g.read, "inputs": g.inputs, "status": g.status, "analyzers": g.analyzers,
        "nodes": g.nodes, "edges": g.edges, "gaps": g.gaps});
    format!("sha256:{:x}", Sha256::digest(body.to_string().as_bytes()))
}

fn build(root: &Path, langs: &[String], out: &Path) -> Result<Value, String> {
    let root = root
        .canonicalize()
        .map_err(|e| format!("{}: {e}", root.display()))?;
    let commit = head(&root)?;
    let files = tracked(&root)?;
    let dirty_paths = dirty_paths(&root)?;
    let langs = if langs.is_empty() {
        detect(&root, &files)
    } else {
        langs.to_vec()
    };
    if langs.is_empty() {
        return Err("no supported language detected; pass --lang".into());
    }
    let (mut analyzers, mut nodes, mut edges, mut gaps) = (vec![], vec![], vec![], vec![]);
    let mut read: Vec<String> = vec![];
    let out = output_path(&root, out)?;
    let scanned = scan_root(&root, &files, &langs);
    for lang in &langs {
        if let Err(e) = &scanned {
            // No tool runs once the scan fails; the version stays unknown.
            let mut a = Analyzer::new(lang, "-", &[]);
            a.error = Some(format!("ROOT scan: {e}"));
            analyzers.push(a);
            continue;
        }
        if !LANGS.contains(&lang.as_str()) {
            return Err(format!("unsupported language: {lang}"));
        }
        let (mut a, result) = isolated(&root, lang);
        match result {
            Ok(p) => {
                a.status = "PASS".into();
                nodes.extend(p.nodes);
                edges.extend(p.edges);
                gaps.extend(p.gaps);
                read.extend(p.read);
            }
            // A failed analyzer contributes nothing so a partial graph is never presented as complete.
            Err(e) => {
                a.status = "FAIL".into();
                a.error = Some(e);
            }
        }
        analyzers.push(a);
    }
    read.sort();
    read.dedup();
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
        inputs: if scanned.is_ok() {
            input_fingerprint(&root, &langs, &read)?
        } else {
            String::new()
        },
        read,
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
    write_replacing(&out, &(text + "\n"))?;
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

fn impact(
    g: &Graph,
    root: &Path,
    head: &str,
    worktree_dirty: bool,
    inputs: &str,
    changed: &[String],
    depth: Option<usize>,
) -> Value {
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
    // Uncommitted edits can add or drop imports the graph of HEAD does not have.
    if worktree_dirty {
        reasons.push("working_tree_differs_from_head");
    }
    // Files the analyzers read but git does not track (ignored sources) can change too.
    if g.inputs != inputs {
        reasons.push("analyzer_inputs_changed");
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
    // A gap marks a file whose dependencies static analysis could not fully resolve, so it may
    // depend on an impacted file without an edge. Gaps outside the impacted set are handed to LLM
    // review; until that review is recorded the static result alone is not complete.
    let impacted_files: BTreeSet<&str> = dist
        .keys()
        .flat_map(|id| by_id[id].files.iter().map(String::as_str))
        .chain(unmapped.iter().map(String::as_str))
        .collect();
    let llm_review: Vec<&Gap> = g
        .gaps
        .iter()
        .filter(|gap| {
            gap.file
                .as_deref()
                .is_none_or(|f| !impacted_files.contains(f))
        })
        .collect();
    if !llm_review.is_empty() {
        reasons.push("gaps_need_llm_review");
    }
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
        "impacted": impacted, "llm_review": llm_review, "gaps": g.gaps,
        "limits": ["static source dependencies only",
            "llm_review lists gaps outside the impacted files: an LLM reviewer must read each site and record whether it reaches an impacted file before the result is used as complete",
            "runtime, configuration, data and dynamic dispatch paths are not included",
            "a missing edge is not evidence of no impact",
            "PASS means the graph was current and every changed file was mapped, not that unlisted code is unaffected"]})
}

fn usage() -> String {
    "usage: taeguk graph build ROOT --out GRAPH.json [--lang elixir|go|python|rust]... | taeguk graph impact GRAPH.json ROOT (--changed PATH... | --git-diff BASE) [--depth N]".into()
}

pub(crate) fn run(args: &[String]) -> Result<Value, String> {
    match args {
        [cmd, lang, root] if cmd == "__analyze" => {
            let (a, result) = analyze(Path::new(root), lang)?;
            let (partial, error) = match result {
                Ok(p) => (Some(p), None),
                Err(e) => (None, Some(e)),
            };
            Ok(json!({"analyzer": a, "partial": partial, "error": error}))
        }
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
            let dirty = !dirty_paths(root)?.is_empty();
            let langs: Vec<String> = g.analyzers.iter().map(|a| a.lang.clone()).collect();
            let inputs = input_fingerprint(root, &langs, &g.read).unwrap_or_default();
            Ok(impact(
                &g,
                root,
                &head(root)?,
                dirty,
                &inputs,
                &changed,
                depth,
            ))
        }
        _ => Err(usage()),
    }
}
