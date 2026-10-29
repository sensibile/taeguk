use super::{Analyzer, Edge, Gap, Node, Partial, TempDir, exec, read_source, tool_version};
use std::{collections::BTreeSet, env, fs, path::Path, process::Command, time::UNIX_EPOCH};

pub(super) fn analyze(root: &Path, tracked: &[String]) -> (Analyzer, Result<Partial, String>) {
    let mut a = Analyzer::new(
        "elixir",
        "mix",
        &[
            "mix",
            "xref",
            "graph",
            "--no-compile",
            "--no-deps-check",
            "--format",
            "dot",
            "--output",
            "<tmp>/xref.dot",
        ],
    );
    a.version = tool_version(root, "elixir");
    let result = run(root, tracked, &mut a);
    (a, result)
}

fn run(root: &Path, tracked: &[String], a: &mut Analyzer) -> Result<Partial, String> {
    let (_, mix) = read_source(root, &root.join("mix.exs"))?;
    if mix.contains("apps_path:") {
        return Err("umbrella projects are not supported; build each app separately".into());
    }
    let mix_env = env::var("MIX_ENV").unwrap_or_else(|_| "dev".into());
    let build = root.join("_build").join(&mix_env);
    // The build is copied, so it must be the project's own directory, not a link out of ROOT.
    if let Ok(real) = build.canonicalize()
        && !real.starts_with(root)
    {
        return Err(format!("_build/{mix_env} resolves outside ROOT"));
    }
    if !build.is_dir() {
        // Compiling here would create _build/deps in the target repository, so require an existing build.
        return Err(format!(
            "_build/{mix_env} not found; compile the project before building the graph"
        ));
    }
    // xref reads the existing manifest without compiling, so a build older than the sources is stale.
    let app = mix
        .split("app:")
        .nth(1)
        .and_then(|r| r.trim_start().strip_prefix(':'))
        .map(|r| {
            r.chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect::<String>()
        })
        .filter(|a| !a.is_empty())
        .ok_or("cannot find app name in mix.exs")?;
    let manifest = build.join("lib").join(&app).join(".mix/compile.elixir");
    let built = modified(&manifest).ok_or_else(|| {
        format!(
            "_build/{mix_env}/lib/{app}/.mix/compile.elixir not found; compile the project first"
        )
    })?;
    let newer: Vec<&String> = tracked
        .iter()
        .filter(|f| f.ends_with(".ex") || f.ends_with(".exs"))
        .filter(|f| !f.starts_with("deps/") && !f.starts_with("test/"))
        .filter(|f| modified(&root.join(f)).is_some_and(|m| m > built))
        .collect();
    if !newer.is_empty() {
        return Err(format!(
            "_build/{mix_env} is older than {} source file(s), e.g. {}; compile the project first",
            newer.len(),
            newer[0]
        ));
    }
    let tmp = TempDir::new("elixir")?;
    copy_tree(root, &build, &tmp.0.join("build"))?;
    a.command.insert(
        0,
        format!("MIX_ENV={mix_env} MIX_BUILD_PATH=<tmp copy of _build/{mix_env}>"),
    );
    let dot = tmp.0.join("xref.dot");
    let o = exec(
        Command::new("mix")
            .args([
                "xref",
                "graph",
                "--no-compile",
                "--no-deps-check",
                "--format",
                "dot",
                "--output",
            ])
            .arg(&dot)
            .current_dir(root)
            .env("MIX_ENV", &mix_env)
            .env("MIX_BUILD_PATH", tmp.0.join("build")),
    )?;
    a.exit_code = o.code;
    if o.code != Some(0) {
        let text = format!("{}\n{}", String::from_utf8_lossy(&o.stdout), o.stderr);
        let lines: Vec<_> = text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect();
        return Err(format!(
            "mix xref graph failed: {}",
            lines[lines.len().saturating_sub(5)..].join(" | ")
        ));
    }
    let text = fs::read_to_string(&dot).map_err(|e| format!("xref output: {e}"))?;
    let mut out = parse_dot(&text)?;
    // The manifest still lists sources deleted since the last compile; their edges are stale.
    let current: BTreeSet<&str> = tracked.iter().map(String::as_str).collect();
    let gone: Vec<&str> = out
        .nodes
        .iter()
        .map(|n| n.files[0].as_str())
        .filter(|f| !current.contains(f) || !root.join(f).is_file())
        .collect();
    if !gone.is_empty() {
        return Err(format!(
            "_build/{mix_env} lists {} source file(s) that are no longer tracked, e.g. {}; compile the project first",
            gone.len(),
            gone[0]
        ));
    }
    for n in &out.nodes {
        let file = &n.files[0];
        let (_, source) = read_source(root, &root.join(file))?;
        for (i, line) in source.lines().enumerate() {
            if DYNAMIC.iter().any(|d| calls(line, d)) || variable_call(line) {
                out.gaps.push(Gap::new(
                    "elixir",
                    "dynamic_call",
                    Some(file),
                    format!("line {}: runtime dispatch is not observed by xref", i + 1),
                ));
            }
        }
    }
    Ok(out)
}

/// Calls whose target module is chosen at runtime, so xref records no edge for them.
const DYNAMIC: [&str; 4] = [
    "apply",
    "Module.concat",
    "String.to_atom",
    "String.to_existing_atom",
];

/// Whether `line` calls a function on a variable (`mod.execute()`, `state.mod.run(x)`): the
/// module is a runtime value, so xref cannot record the edge. Without parentheses
/// (`mod.execute x`) an argument must follow; map field access (`state.count + 1`) and an
/// anonymous call `fun.(x)` are not matched.
fn variable_call(line: &str) -> bool {
    let code = line.split('#').next().unwrap_or(line);
    let b = code.as_bytes();
    let ident = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
    (0..b.len()).any(|i| {
        if b[i] != b'.' || i == 0 || !ident(b[i - 1]) {
            return false;
        }
        // The receiver starts with a lowercase letter or `_` and is not an atom (`:ets.new`).
        let mut start = i;
        while start > 0 && ident(b[start - 1]) {
            start -= 1;
        }
        let head = b[start];
        let atom = start > 0 && b[start - 1] == b':';
        if !(head.is_ascii_lowercase() || head == b'_') || atom {
            return false;
        }
        let mut end = i + 1;
        if end >= b.len() || !(b[end].is_ascii_lowercase() || b[end] == b'_') {
            return false;
        }
        while end < b.len() && (ident(b[end]) || b[end] == b'?' || b[end] == b'!') {
            end += 1;
        }
        if end < b.len() && b[end] == b'(' {
            return true;
        }
        // Without parentheses (`mod.execute x`) an argument follows after spaces; an operator,
        // separator or block keyword there means field access instead.
        let rest = code[end..].trim_start_matches([' ', '\t']);
        if rest.len() == code.len() - end {
            return false;
        }
        let word: String = rest
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        let starts_arg = rest
            .chars()
            .next()
            .is_some_and(|c| c.is_alphanumeric() || "_:\"'[{%@&".contains(c));
        starts_arg
            && ![
                "do", "end", "and", "or", "when", "in", "not", "else", "after", "rescue", "catch",
            ]
            .contains(&word.as_str())
    })
}

/// Whether `line` calls `name`, with or without parentheses (`apply(m, f, a)`, `apply m, f, a`,
/// `Kernel.apply ...`, `|> apply(...)`): the name is not part of a longer identifier and is
/// followed by `(` or by whitespace and an argument.
fn calls(line: &str, name: &str) -> bool {
    let ident = |c: char| c.is_alphanumeric() || c == '_' || c == '?' || c == '!';
    line.match_indices(name).any(|(i, _)| {
        let before = line[..i].chars().next_back();
        let after = line[i + name.len()..].trim_start_matches([' ', '\t']);
        let spaced = line[i + name.len()..].starts_with([' ', '\t']);
        before.is_none_or(|c| !ident(c))
            && (line[i + name.len()..].starts_with('(')
                || (spaced
                    && after
                        .chars()
                        .next()
                        .is_some_and(|c| !matches!(c, '=' | '|' | '-' | '<' | '>' | ')' | ','))))
    })
}

/// Modification time in whole seconds; Mix writes the manifest time at that resolution,
/// so a source saved earlier in the same second must not look newer.
fn modified(path: &Path) -> Option<u64> {
    let t = fs::metadata(path).and_then(|m| m.modified()).ok()?;
    t.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs())
}

/// Parses `mix xref graph --format dot`: `"a"` node lines and `"a" -> "b" [label="(compile)"]` edges.
fn parse_dot(text: &str) -> Result<Partial, String> {
    let mut files = BTreeSet::new();
    let mut out = Partial::default();
    for line in text.lines().map(str::trim) {
        if !line.starts_with('"') {
            continue;
        }
        let quoted: Vec<&str> = line.split('"').collect();
        match quoted.as_slice() {
            [_, from, _] => {
                files.insert(from.to_string());
            }
            [_, from, arrow, to, rest @ ..] if arrow.trim() == "->" => {
                let label = rest.get(1).copied().unwrap_or("");
                let kind = match label {
                    "(compile)" => "compile",
                    "(export)" => "export",
                    "" => "runtime",
                    other => return Err(format!("unknown xref label: {other}")),
                };
                files.insert(from.to_string());
                files.insert(to.to_string());
                out.edges.push(Edge::new(
                    &format!("elixir:{from}"),
                    &format!("elixir:{to}"),
                    kind,
                ));
            }
            _ => return Err(format!("unexpected xref line: {line}")),
        }
    }
    out.nodes = files
        .into_iter()
        .map(|f| Node::new("elixir", f.clone(), vec![f]))
        .collect();
    Ok(out)
}

/// Copies a build tree; symlinks are recreated as absolute links to their original targets.
/// The build is ignored by git, so the ROOT scan does not cover it: links must stay inside ROOT
/// and point at files or directories, and special files are refused (copying a FIFO blocks).
fn copy_tree(root: &Path, from: &Path, to: &Path) -> Result<(), String> {
    fs::create_dir_all(to).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(from).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let (src, dst) = (entry.path(), to.join(entry.file_name()));
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        if kind.is_symlink() {
            let target = fs::read_link(&src).map_err(|e| e.to_string())?;
            let target = if target.is_absolute() {
                target
            } else {
                from.join(target)
            };
            let real = target
                .canonicalize()
                .map_err(|e| format!("{}: {e}", src.display()))?;
            if !real.starts_with(root) || !(real.is_file() || real.is_dir()) {
                return Err(format!(
                    "build link {} must point to a file or directory inside ROOT",
                    src.display()
                ));
            }
            #[cfg(unix)]
            std::os::unix::fs::symlink(&target, &dst).map_err(|e| e.to_string())?;
            #[cfg(not(unix))]
            return Err(format!(
                "symlinked build entries are not supported here: {}",
                src.display()
            ));
        } else if kind.is_dir() {
            copy_tree(root, &src, &dst)?;
        } else if !kind.is_file() {
            return Err(format!(
                "build entry {} is not a regular file",
                src.display()
            ));
        } else {
            fs::copy(&src, &dst).map_err(|e| format!("{}: {e}", src.display()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dot_labels_become_edge_kinds() {
        let p = parse_dot("digraph \"xref graph\" {\n  \"lib/a.ex\"\n  \"lib/a.ex\" -> \"lib/b.ex\" [label=\"(compile)\"]\n  \"lib/b.ex\" -> \"lib/c.ex\"\n  \"lib/c.ex\" -> \"lib/a.ex\" [label=\"(export)\"]\n}\n").unwrap();
        let kinds: Vec<_> = p.edges.iter().map(|e| e.kind.as_str()).collect();
        assert_eq!(kinds, ["compile", "runtime", "export"]);
        assert_eq!(p.nodes.len(), 3);
        assert!(parse_dot("\"a\" -> \"b\" [label=\"(weird)\"]").is_err());
    }
    #[test]
    fn dynamic_calls_with_or_without_parentheses() {
        for line in [
            "apply(m, :run, [])",
            "def call(m), do: apply m, :run, []",
            "Kernel.apply m, :run, []",
            "m |> Module.concat(Sub)",
            "String.to_atom name",
        ] {
            assert!(DYNAMIC.iter().any(|d| calls(line, d)), "{line}");
        }
        for line in [
            "applyx(m)",
            "reapply(m)",
            "apply = 1",
            "x.apply_all(m)",
            "# applies",
        ] {
            assert!(!DYNAMIC.iter().any(|d| calls(line, d)), "{line}");
        }
        for line in [
            "def run(mod), do: mod.execute()",
            "state.mod.run(x)",
            "_m.f(1)",
            "def run(mod, x), do: mod.execute x",
            "mod.start :normal, []",
        ] {
            assert!(variable_call(line), "{line}");
        }
        for line in [
            "Enum.map(xs, f)",
            ":ets.new(:t, [])",
            "fun.(x)",
            "state.count + 1",
            "if state.ok do",
            "x = map.value",
            "f(map.a, map.b)",
            "state.items |> Enum.map(&f/1)",
            "# mod.run()",
            "Fx.B.b()",
        ] {
            assert!(!variable_call(line), "{line}");
        }
    }
}
