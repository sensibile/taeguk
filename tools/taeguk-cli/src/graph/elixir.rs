use super::{
    Analyzer, Edge, Gap, Node, Partial, TempDir, check_source, exec_input, read_source, relative,
    tool_version,
};
use std::{collections::BTreeSet, env, fs, path::Path, process::Command, time::UNIX_EPOCH};

/// Reads the Mix compile manifest with the `elixir` executable only. `mix` is not run: it
/// evaluates the project's `mix.exs`, which is code from the target repository. The manifest
/// records, per source file, the modules it references at compile time, through exports and at
/// runtime; that is what `mix xref graph` reports. Only manifest version 35 (Elixir 1.20) is
/// understood; another version fails.
const SCRIPT: &str = r#"
# The manifest path arrives on stdin, never as an argument.
path = IO.read(:stdio, :eof) |> String.trim_trailing("\n")
# Only the parts read below are used, and each is checked to be plain data of the expected
# shape before it is enumerated: enumerating a struct (a Stream, a File.Stream) or calling a
# function would run code that came from the file. Other fields (warnings hold diagnostic
# structs) are never touched.
refuse = fn what ->
  IO.puts(:stderr, "manifest #{what} is not plain data of the expected shape; refused")
  System.halt(4)
end
plain_map = fn t -> is_map(t) and not Map.has_key?(t, :__struct__) end
atoms = fn
  r when is_list(r) -> if Enum.all?(r, &is_atom/1), do: r, else: refuse.("references")
  r when is_map(r) ->
    if plain_map.(r) and Enum.all?(Map.keys(r), &is_atom/1), do: Map.keys(r), else: refuse.("references")
  _ -> refuse.("references")
end
case :erlang.binary_to_term(File.read!(path)) do
  {35, modules, sources, _, _, _, _, _, _, _, _} ->
    if not (plain_map.(modules) and plain_map.(sources)), do: refuse.("index")
    files = fn m ->
      case modules do
        %{^m => m_info} when is_tuple(m_info) and tuple_size(m_info) > 2 and elem(m_info, 0) == :module ->
          srcs = elem(m_info, 2)
          if is_list(srcs) and Enum.all?(srcs, &is_binary/1), do: srcs, else: refuse.("module sources")
        _ -> []
      end
    end
    for {src, info} <- sources, is_binary(src), is_tuple(info), tuple_size(info) > 6,
        elem(info, 0) == :source do
      IO.puts("node\t" <> src)
      edges =
        for {kind, i} <- [compile: 4, export: 5, runtime: 6], m <- atoms.(elem(info, i)),
            to <- files.(m), to != src, reduce: %{} do
          acc -> Map.put_new(acc, to, kind)
        end
      for {to, kind} <- edges, do: IO.puts("edge\t#{src}\t#{to}\t#{kind}")
    end
  other ->
    # Only an integer version is printed; inspecting arbitrary terms could run their code.
    version = if is_tuple(other) and tuple_size(other) > 0 and is_integer(elem(other, 0)), do: elem(other, 0), else: "unknown"
    IO.puts(:stderr, "unsupported Mix manifest version #{version}")
    System.halt(3)
end
"#;

/// Finds, with Elixir's own parser, the calls whose target module is chosen at runtime, which
/// the manifest cannot record. `Code.string_to_quoted` only parses; nothing is evaluated, and
/// strings and comments are not code to it. Allow-list: a remote call `recv.fun` is static only
/// when `recv` is an alias (`Enum`), an atom (`:ets`) or `__MODULE__`; any other receiver (a
/// variable, a module attribute, a parenthesized expression, map field access, which looks the
/// same) is a gap, and so are `apply`, `Kernel.apply`, `:erlang.apply`, `Module.concat`,
/// `Module.safe_concat`, `String.to_atom` and `String.to_existing_atom`, also through `alias ...,
/// as:`, as imported local calls or as captures; so is every anonymous call `f.(...)`. A file that does not
/// parse is a gap as a whole. Files arrive on stdin, one `ROOT-relative\tabsolute` per line, so
/// no repository file name is ever a command-line argument.
const SCAN: &str = r##"
gap = fn rel, line, detail -> IO.puts("gap\t#{rel}\t#{line || 0}\t#{detail}") end
dynamic = [
  {[:Kernel], :apply}, {[:Module], :concat}, {[:Module], :safe_concat},
  {[:String], :to_atom}, {[:String], :to_existing_atom}
]
for line <- String.split(IO.read(:stdio, :eof), "\n", trim: true),
    [rel, file] = String.split(line, "\t", parts: 2) do
  case Code.string_to_quoted(File.read!(file), columns: false, emit_warnings: false) do
    {:ok, ast} ->
      # `alias Kernel, as: K` and `alias Mod.{A, B}` make short names stand for full ones.
      aliases =
        ast
        |> Macro.prewalk(%{}, fn
          {kind, _, [{:__aliases__, _, parts} | rest]} = node, acc
          when kind in [:alias, :require] and is_list(parts) ->
            as =
              case rest do
                [opts] when is_list(opts) ->
                  case Keyword.get(opts, :as) do
                    {:__aliases__, _, [name]} -> name
                    _ -> List.last(parts)
                  end
                _ -> List.last(parts)
              end
            {node, Map.update(acc, as, [parts], &Enum.uniq([parts | &1]))}
          {:alias, _, [{{:., _, [{:__aliases__, _, base}, :{}]}, _, children} | _]} = node, acc ->
            acc =
              Enum.reduce(children, acc, fn
                {:__aliases__, _, parts}, acc ->
                  Map.update(acc, List.last(parts), [base ++ parts], &Enum.uniq([base ++ parts | &1]))
                _, acc -> acc
              end)
            {node, acc}
          # `alias :"Elixir.Kernel", as: K` names the module by atom; any other target that is
          # not a plain alias cannot be read here, so its short name becomes ambiguous.
          {kind, _, [target | rest]} = node, acc when kind in [:alias, :require] ->
            parts =
              case target do
                atom when is_atom(atom) ->
                  case Atom.to_string(atom) do
                    # An atom is an absolute name; the leading :Elixir keeps it from expansion.
                    "Elixir." <> name -> [:Elixir | Enum.map(String.split(name, "."), &String.to_atom/1)]
                    _ -> [:Elixir, atom]
                  end
                _ -> nil
              end
            as =
              case rest do
                [opts] when is_list(opts) ->
                  case Keyword.get(opts, :as) do
                    {:__aliases__, _, [name]} -> name
                    _ -> parts && List.last(parts)
                  end
                _ -> parts && List.last(parts)
              end
            cond do
              is_nil(as) -> {node, acc}
              is_nil(parts) -> {node, Map.put(acc, as, [[:"?1"], [:"?2"]])}
              true -> {node, Map.update(acc, as, [parts], &Enum.uniq([parts | &1]))}
            end
          node, acc -> {node, acc}
        end)
        |> elem(1)
      # Module names are compared in one form: `Elixir.Kernel` and `:"Elixir.Kernel"` are
      # `Kernel`, and alias chains (`alias K, as: K2`) are followed up to 8 steps. A short name
      # aliased to different modules in one file depends on where the call sits, which is not
      # tracked, so it is ambiguous.
      strip = fn [:Elixir | rest] -> rest; parts -> parts end
      expand = fn
        # `Elixir.Kernel` is absolute: local aliases do not apply to it.
        [:Elixir | rest] -> rest
        parts ->
        Enum.reduce_while(1..8, parts, fn _, [h | t] = acc ->
          case aliases do
            # An absolute target (`Elixir.X`, an atom) is final; a relative one may be an alias.
            %{^h => [[:Elixir | _] = full]} -> {:halt, strip.(full) ++ t}
            %{^h => [full]} when full != [h] -> {:cont, full ++ t}
            %{^h => [_, _ | _]} -> {:halt, :ambiguous}
            _ -> {:halt, acc}
          end
        end)
      end
      Macro.prewalk(ast, fn
        {{:., meta, [recv, fun]}, _, args} = node when is_atom(fun) and is_list(args) ->
          static =
            match?({:__aliases__, _, _}, recv) or is_atom(recv) or
              match?({:__MODULE__, _, ctx} when is_atom(ctx), recv)
          known =
            case recv do
              {:__aliases__, _, parts} when is_list(parts) and parts != [] ->
                case expand.(parts) do
                  :ambiguous -> true
                  full -> {full, fun} in dynamic
                end
              :erlang -> fun == :apply
              atom when is_atom(atom) ->
                case Atom.to_string(atom) do
                  "Elixir." <> name ->
                    {Enum.map(String.split(name, "."), &String.to_atom/1), fun} in dynamic
                  _ -> false
                end
              _ -> false
            end
          cond do
            not static -> gap.(rel, meta[:line], "call on a runtime value (#{fun})")
            known -> gap.(rel, meta[:line], "#{fun} picks its module at runtime")
            true -> :ok
          end
          node
        # An anonymous function call `f.(...)` runs whatever `f` holds (`&apply/3`, a closure
        # over a runtime module), which is not known here.
        {{:., meta, [_callee]}, _, args} = node when is_list(args) ->
          gap.(rel, meta[:line], "anonymous function call; its target is not known")
          node
        # Local calls (and captures such as `&apply/3`) that `import Kernel/Module/String` make
        # dynamic: `apply`, `concat`, `safe_concat`, `to_atom`, `to_existing_atom`.
        {name, meta, args} = node
        when name in [:apply, :concat, :safe_concat, :to_atom, :to_existing_atom] and
               (is_list(args) or is_atom(args)) ->
          gap.(rel, meta[:line], "#{name} may pick its module at runtime")
          node
        node ->
          node
      end)
    {:error, _} ->
      gap.(rel, 0, "file does not parse; its calls are not analyzed")
  end
end
"##;

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
    let o = exec_input(
        Command::new("elixir")
            .args(["-e", SCRIPT])
            .current_dir(&tmp.0),
        real.to_string_lossy().as_bytes(),
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
    // Both the path read through (a link can be repointed) and the file it reached.
    out.read.push(shown.clone());
    out.read.push(relative(root, &real).unwrap_or(shown));
    // Every source is checked (regular file inside ROOT, size bound) before elixir reads it.
    let mut list = String::new();
    for n in &out.nodes {
        let file = &n.files[0];
        let real = check_source(root, &root.join(file))?;
        if file.contains(['\t', '\n']) || real.to_string_lossy().contains(['\t', '\n']) {
            return Err(format!("{file:?}: tab or newline in a source path"));
        }
        list += &format!("{file}\t{}\n", real.display());
    }
    let o = exec_input(
        Command::new("elixir")
            .args(["-e", SCAN])
            .current_dir(&tmp.0),
        list.as_bytes(),
    )?;
    if o.code != Some(0) {
        return Err(format!("scanning sources failed: {}", o.stderr.trim()));
    }
    for line in String::from_utf8_lossy(&o.stdout).lines() {
        let [_, file, line_no, detail] = line.splitn(4, '\t').collect::<Vec<_>>()[..] else {
            return Err(format!("unexpected source scan line: {line}"));
        };
        out.gaps.push(Gap::new(
            "elixir",
            "dynamic_call",
            Some(file),
            format!("line {line_no}: {detail}; not observed in the manifest"),
        ));
    }
    Ok(out)
}

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
}
