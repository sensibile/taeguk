use super::{Analyzer, Edge, Node, Partial, TempDir, exec, version};
use std::{collections::BTreeSet, env, fs, path::Path, process::Command};

pub(super) fn analyze(root: &Path) -> (Analyzer, Result<Partial, String>) {
    let mut a = Analyzer::new(
        "elixir",
        "mix",
        &[
            "mix",
            "xref",
            "graph",
            "--format",
            "dot",
            "--output",
            "<tmp>/xref.dot",
        ],
    );
    a.version = version("mix", &["--version"]);
    let result = run(root, &mut a);
    (a, result)
}

fn run(root: &Path, a: &mut Analyzer) -> Result<Partial, String> {
    let mix = fs::read_to_string(root.join("mix.exs")).map_err(|e| format!("mix.exs: {e}"))?;
    if mix.contains("apps_path:") {
        return Err("umbrella projects are not supported; build each app separately".into());
    }
    let mix_env = env::var("MIX_ENV").unwrap_or_else(|_| "dev".into());
    let build = root.join("_build").join(&mix_env);
    if !build.is_dir() {
        // Compiling here would create _build/deps in the target repository, so require an existing build.
        return Err(format!(
            "_build/{mix_env} not found; compile the project before building the graph"
        ));
    }
    let tmp = TempDir::new("elixir")?;
    copy_tree(&build, &tmp.0.join("build"))?;
    a.command.insert(
        0,
        format!("MIX_ENV={mix_env} MIX_BUILD_PATH=<tmp copy of _build/{mix_env}>"),
    );
    let dot = tmp.0.join("xref.dot");
    let o = exec(
        Command::new("mix")
            .args(["xref", "graph", "--format", "dot", "--output"])
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
    parse_dot(&text)
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
fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
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
            #[cfg(unix)]
            std::os::unix::fs::symlink(&target, &dst).map_err(|e| e.to_string())?;
            #[cfg(not(unix))]
            return Err(format!(
                "symlinked build entries are not supported here: {}",
                src.display()
            ));
        } else if kind.is_dir() {
            copy_tree(&src, &dst)?;
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
}
