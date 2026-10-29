use super::{
    Analyzer, Edge, Gap, Node, Partial, TempDir, check_source, exec, relative, tool_version,
};
use serde::Deserialize;
use std::{collections::BTreeSet, fs, path::Path, process::Command};

#[derive(Deserialize, Default)]
#[serde(rename_all = "PascalCase", default)]
struct Package {
    import_path: String,
    name: String,
    for_test: String,
    dir: String,
    standard: bool,
    module: Option<Module>,
    go_files: Vec<String>,
    cgo_files: Vec<String>,
    test_go_files: Vec<String>,
    x_test_go_files: Vec<String>,
    ignored_go_files: Vec<String>,
    embed_patterns: Vec<String>,
    test_embed_patterns: Vec<String>,
    x_test_embed_patterns: Vec<String>,
    imports: Vec<String>,
    test_imports: Vec<String>,
    x_test_imports: Vec<String>,
    error: Option<PackageError>,
    deps_errors: Vec<PackageError>,
}
#[derive(Deserialize, Default)]
#[serde(rename_all = "PascalCase", default)]
struct Module {
    main: bool,
    go_mod: String,
}
#[derive(Deserialize, Default)]
#[serde(rename_all = "PascalCase", default)]
struct PackageError {
    err: String,
}

const COMMAND: [&str; 9] = [
    "go", "-C", "<ROOT>", "list", "-e", "-deps", "-test", "-json", "./...",
];

pub(super) fn analyze(root: &Path) -> (Analyzer, Result<Partial, String>) {
    let mut a = Analyzer::new("go", "go", &COMMAND);
    a.version = tool_version("go");
    let result = run(root, &mut a);
    (a, result)
}

fn run(root: &Path, a: &mut Analyzer) -> Result<Partial, String> {
    // -test also loads test-only imports, so a missing one fails like any other import.
    // No network or toolchain download: missing modules must fail instead of being fetched.
    // GOPRIVATE/GONOPROXY would otherwise send matching modules straight to their VCS hosts.
    // `go -C ROOT` runs from a directory outside ROOT, so a version manager picks go from the
    // user's environment, not from files in the target repository.
    let dir = TempDir::new("go")?;
    // go looks for go.work in ROOT and every directory above it; only one inside ROOT is used.
    let work = root.join("go.work");
    let gowork = if fs::symlink_metadata(&work).is_ok() {
        check_source(root, &work)?;
        work.into_os_string()
    } else {
        "off".into()
    };
    let o = exec(
        Command::new("go")
            .arg("-C")
            .arg(root)
            .args(&COMMAND[3..])
            .current_dir(&dir.0)
            .env("GOPROXY", "off")
            .env("GOPRIVATE", "")
            .env("GONOPROXY", "")
            .env("GONOSUMDB", "")
            .env("GOSUMDB", "off")
            .env("GOVCS", "*:off")
            .env("GOTOOLCHAIN", "local")
            .env("GOFLAGS", "-mod=readonly")
            .env("GOWORK", gowork),
    )?;
    a.exit_code = o.code;
    let mut packages = Vec::new();
    for p in serde_json::Deserializer::from_slice(&o.stdout).into_iter::<Package>() {
        packages.push(p.map_err(|e| format!("cannot parse go list output: {e}"))?);
    }
    // go's messages can quote files it read, including ones reached outside ROOT (a replaced
    // module's go.mod), so no message text is kept. The failure names packages only when the
    // name itself comes from ROOT: a package inside ROOT, or one that such a package imports.
    // A package counts as from ROOT only when its directory and every file go read for it
    // resolve inside ROOT (an ignored `.go` link could bring an outside file's imports).
    let inside = |p: &Package| {
        Path::new(&p.dir).starts_with(root)
            && p.module.as_ref().is_none_or(|m| {
                m.go_mod.is_empty() || check_source(root, Path::new(&m.go_mod)).is_ok()
            })
            && [
                &p.go_files,
                &p.cgo_files,
                &p.test_go_files,
                &p.x_test_go_files,
                &p.ignored_go_files,
            ]
            .into_iter()
            .flatten()
            .all(|f| check_source(root, &Path::new(&p.dir).join(f)).is_ok())
    };
    let named: BTreeSet<&str> = packages
        .iter()
        .filter(|p| inside(p))
        .flat_map(|p| {
            std::iter::once(p.import_path.as_str())
                .chain(p.imports.iter().map(String::as_str))
                .chain(p.test_imports.iter().map(String::as_str))
                .chain(p.x_test_imports.iter().map(String::as_str))
        })
        .collect();
    let failing: Vec<&Package> = packages
        .iter()
        .filter(|p| !p.standard && (p.error.is_some() || !p.deps_errors.is_empty()))
        .collect();
    if o.code != Some(0) || !failing.is_empty() {
        let mut shown: Vec<&str> = failing
            .iter()
            .map(|p| p.import_path.as_str())
            .filter(|n| named.contains(n))
            .collect();
        shown.sort();
        shown.dedup();
        let hidden = failing.len()
            - failing
                .iter()
                .filter(|p| named.contains(p.import_path.as_str()))
                .count();
        return Err(format!(
            "go list failed (exit {:?}): package errors in [{}]{}; run `go list -e -deps -test ./...` in ROOT for the messages",
            o.code,
            shown.join(", "),
            if hidden > 0 {
                format!(" and {hidden} more")
            } else {
                String::new()
            }
        ));
    }
    let main: Vec<&Package> = packages
        .iter()
        // Test variants ("p [p.test]") and generated test mains repeat the packages listed plainly.
        // A generated test main is recognized by its generated source in the build cache (an
        // absolute file name), not by a `.test` suffix a real package path may also have.
        .filter(|p| !p.standard && p.module.as_ref().is_some_and(|m| m.main))
        .filter(|p| p.for_test.is_empty())
        .filter(|p| {
            !(p.name == "main"
                && p.import_path.ends_with(".test")
                && p.go_files.iter().any(|f| Path::new(f).is_absolute()))
        })
        .collect();
    let ids: BTreeSet<&str> = main.iter().map(|p| p.import_path.as_str()).collect();
    let mut out = Partial::default();
    for p in main {
        let dir = Path::new(&p.dir);
        let rel_dir = relative(root, dir)
            .ok_or_else(|| format!("package {} is outside ROOT", p.import_path))?;
        let file = |f: &String| {
            if rel_dir.is_empty() {
                f.clone()
            } else {
                format!("{rel_dir}/{f}")
            }
        };
        // The module's go.mod names the package; it must resolve inside ROOT like the sources.
        if let Some(m) = p.module.as_ref().filter(|m| !m.go_mod.is_empty()) {
            check_source(root, Path::new(&m.go_mod))?;
        }
        // Untracked files in a package directory are read by go too; they must also stay inside ROOT.
        for f in [
            &p.go_files,
            &p.cgo_files,
            &p.test_go_files,
            &p.x_test_go_files,
            &p.ignored_go_files,
        ]
        .into_iter()
        .flatten()
        {
            check_source(root, &dir.join(f))?;
        }
        let mut files: Vec<String> = [
            &p.go_files,
            &p.cgo_files,
            &p.test_go_files,
            &p.x_test_go_files,
            &p.ignored_go_files,
        ]
        .into_iter()
        .flatten()
        .map(file)
        .collect();
        files.sort();
        // `//go:embed` reads files of any kind from the package directory down; which ones match
        // can change with files added or removed, so the whole directory tree is an input.
        if [
            &p.embed_patterns,
            &p.test_embed_patterns,
            &p.x_test_embed_patterns,
        ]
        .iter()
        .any(|v| !v.is_empty())
        {
            // `./` is the whole ROOT tree (a package at the module root).
            out.read.push(if rel_dir.is_empty() {
                "./".into()
            } else {
                format!("{rel_dir}/")
            });
        }
        let id = format!("go:{}", p.import_path);
        out.nodes.push(Node::new("go", &p.import_path, files));
        for (kind, imports) in [
            ("import", &p.imports),
            ("test_import", &p.test_imports),
            ("test_import", &p.x_test_imports),
        ] {
            for i in imports.iter().filter(|i| ids.contains(i.as_str())) {
                out.edges.push(Edge::new(&id, &format!("go:{i}"), kind));
            }
        }
        // `plugin.Open` loads code chosen at runtime; that dependency is not in the graph.
        if [&p.imports, &p.test_imports, &p.x_test_imports]
            .iter()
            .any(|v| v.iter().any(|i| i == "plugin"))
        {
            out.gaps.push(Gap::new(
                "go",
                "dynamic_load",
                Some(&rel_dir),
                format!(
                    "package {} imports plugin; plugins it loads are not analyzed",
                    p.import_path
                ),
            ));
        }
        for f in &p.ignored_go_files {
            out.gaps.push(Gap::new("go", "build_constraint_excluded", Some(&file(f)),
                "file excluded by build constraints in the current environment; its imports are not analyzed"));
        }
    }
    Ok(out)
}
