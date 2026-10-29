use super::{
    Analyzer, Edge, Gap, Node, Partial, TempDir, exec, read_source, relative, tool_version,
};
use std::{collections::BTreeSet, env, fs, path::Path, process::Command, time::UNIX_EPOCH};

/// Reads the Mix compile manifest with the `elixir` executable only. `mix` is not run: it
/// evaluates the project's `mix.exs`, which is code from the target repository. The manifest
/// records, per source file, the modules it references at compile time, through exports and at
/// runtime; that is what `mix xref graph` reports. Only manifest version 35 (Elixir 1.20) is
/// understood; another version fails.
const SCRIPT: &str = r#"
[path] = System.argv()
refs = fn r when is_map(r) -> Map.keys(r); r when is_list(r) -> r; _ -> [] end
case :erlang.binary_to_term(File.read!(path)) do
  {35, modules, sources, _, _, _, _, _, _, _, _} ->
    files = fn m ->
      case modules do
        %{^m => m_info} when elem(m_info, 0) == :module -> elem(m_info, 2)
        _ -> []
      end
    end
    for {src, info} <- sources, elem(info, 0) == :source do
      IO.puts("node\t" <> src)
      edges =
        for {kind, i} <- [compile: 4, export: 5, runtime: 6], m <- refs.(elem(info, i)),
            to <- files.(m), to != src, reduce: %{} do
          acc -> Map.put_new(acc, to, kind)
        end
      for {to, kind} <- edges, do: IO.puts("edge\t#{src}\t#{to}\t#{kind}")
    end
  other ->
    IO.puts(:stderr, "unsupported Mix manifest version #{inspect(is_tuple(other) && elem(other, 0))}")
    System.halt(3)
end
"#;

/// Largest compile manifest read.
const MAX_MANIFEST: u64 = 256 << 20;

pub(super) fn analyze(root: &Path, tracked: &[String]) -> (Analyzer, Result<Partial, String>) {
    let mut a = Analyzer::new(
        "elixir",
        "elixir",
        &[
            "elixir",
            "-e",
            "<embedded manifest reader>",
            "_build/<env>/lib/<app>/.mix/compile.elixir",
        ],
    );
    a.version = tool_version("elixir");
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
    if !build.is_dir() {
        // Compiling here would create _build/deps in the target repository, so require an existing build.
        return Err(format!(
            "_build/{mix_env} not found; compile the project before building the graph"
        ));
    }
    // The app name is read from the text of mix.exs, which is never evaluated.
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
    let shown = format!("_build/{mix_env}/lib/{app}/.mix/compile.elixir");
    let real = manifest
        .canonicalize()
        .map_err(|_| format!("{shown} not found; compile the project first"))?;
    let meta = fs::metadata(&real).map_err(|e| format!("{shown}: {e}"))?;
    if !real.starts_with(root) || !meta.is_file() || meta.len() > MAX_MANIFEST {
        return Err(format!(
            "{shown} must be a regular file inside ROOT of at most {MAX_MANIFEST} bytes"
        ));
    }
    // The manifest describes the last compile, so a build older than the sources is stale.
    let built = modified(&real).ok_or_else(|| format!("{shown}: no modification time"))?;
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
    a.command[3] = shown.clone();
    // Run outside ROOT so nothing from the target repository is picked up from the directory.
    let tmp = TempDir::new("elixir")?;
    let o = exec(
        Command::new("elixir")
            .args(["-e", SCRIPT])
            .arg(&real)
            .current_dir(&tmp.0),
    )?;
    a.exit_code = o.code;
    if o.code != Some(0) {
        return Err(format!("reading {shown} failed: {}", o.stderr.trim()));
    }
    let mut out = parse_refs(&String::from_utf8_lossy(&o.stdout))?;
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
    out.read.push(relative(root, &real).unwrap_or(shown));
    for n in &out.nodes {
        let file = &n.files[0];
        let (_, source) = read_source(root, &root.join(file))?;
        for (i, line) in source.lines().enumerate() {
            if DYNAMIC.iter().any(|d| calls(line, d)) || variable_call(line) {
                out.gaps.push(Gap::new(
                    "elixir",
                    "dynamic_call",
                    Some(file),
                    format!(
                        "line {}: runtime dispatch is not observed in the manifest",
                        i + 1
                    ),
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

/// Allow-list for dot access: `.name` is static only after a module alias (`Enum.map`,
/// `Fx.B.b`), an Erlang module atom (`:ets.new`) or a number. After anything else (a variable
/// `mod.run`, a module attribute `@impl.call`, a parenthesized or bracketed expression
/// `(mod).run()`, a string) the receiver may be a module chosen at runtime, which the manifest
/// does not record, so the line is a gap. Map field access looks the same and is included.
/// Anonymous calls `fun.(x)` and ranges `a..b` are not dot access. Comments are not cut off
/// (`#` also appears in strings), so a match there only adds a gap.
fn variable_call(line: &str) -> bool {
    let b = line.as_bytes();
    let ident = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
    (1..b.len()).any(|i| {
        let next = b.get(i + 1).copied();
        if b[i] != b'.'
            || b[i - 1] == b'.'
            || !next.is_some_and(|c| c.is_ascii_lowercase() || c == b'_')
        {
            return false;
        }
        if !ident(b[i - 1]) {
            return true;
        }
        // Walk back over the receiver chain `A.B.c` to its first segment.
        let mut s = i;
        while s > 0 && (ident(b[s - 1]) || b[s - 1] == b'.') {
            s -= 1;
        }
        let head = b[s];
        let atom = s > 0 && b[s - 1] == b':';
        !(head.is_ascii_uppercase() || head.is_ascii_digit() || atom)
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

/// Parses the reader output: `node\tFILE` and `edge\tFROM\tTO\tKIND` lines.
fn parse_refs(text: &str) -> Result<Partial, String> {
    let mut files = BTreeSet::new();
    let mut out = Partial::default();
    for line in text.lines().filter(|l| !l.is_empty()) {
        match line.split('\t').collect::<Vec<_>>().as_slice() {
            ["node", f] => {
                files.insert(f.to_string());
            }
            ["edge", from, to, kind @ ("compile" | "export" | "runtime")] => {
                files.insert(from.to_string());
                files.insert(to.to_string());
                out.edges.push(Edge::new(
                    &format!("elixir:{from}"),
                    &format!("elixir:{to}"),
                    kind,
                ));
            }
            _ => return Err(format!("unexpected manifest reader line: {line}")),
        }
    }
    out.nodes = files
        .into_iter()
        .map(|f| Node::new("elixir", f.clone(), vec![f]))
        .collect();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reader_lines_become_edges() {
        let p = parse_refs("node\tlib/a.ex\nedge\tlib/a.ex\tlib/b.ex\tcompile\nedge\tlib/b.ex\tlib/c.ex\truntime\n").unwrap();
        let kinds: Vec<_> = p.edges.iter().map(|e| e.kind.as_str()).collect();
        assert_eq!(kinds, ["compile", "runtime"]);
        assert_eq!(p.nodes.len(), 3);
        assert!(parse_refs("edge\ta\tb\tweird").is_err());
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
            "def run(mod), do: mod.execute",
            "state.count + 1",
            "x = map.value",
            "\"#{x}\" <> mod.run()",
            "_label = \"#\"; mod.execute()",
            "# mod.run()",
            "@impl.call()",
            "def call(mod), do: (mod).run()",
            "list[0].run()",
        ] {
            assert!(variable_call(line), "{line}");
        }
        for line in [
            "Enum.map(xs, f)",
            ":ets.new(:t, [])",
            "fun.(x)",
            "Fx.B.b()",
            "for i <- 1..10, do: i",
            "x = 1.5",
        ] {
            assert!(!variable_call(line), "{line}");
        }
    }
}
