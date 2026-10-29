use super::{
    Analyzer, Edge, Gap, Node, Partial, TempDir, check_source, exec, relative, tool_version,
};
use serde::Deserialize;
use std::{collections::BTreeSet, path::Path, process::Command};

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
            .env("GOFLAGS", "-mod=readonly"),
    )?;
    a.exit_code = o.code;
    let mut packages = Vec::new();
    for p in serde_json::Deserializer::from_slice(&o.stdout).into_iter::<Package>() {
        packages.push(p.map_err(|e| format!("cannot parse go list output: {e}"))?);
    }
    let mut errors: Vec<String> = packages
        .iter()
        .filter(|p| !p.standard)
        .flat_map(|p| p.error.iter().chain(&p.deps_errors))
        .map(|e| e.err.clone())
        .collect();
    errors.sort();
    errors.dedup();
    if o.code != Some(0) || !errors.is_empty() {
        let detail = if errors.is_empty() {
            o.stderr.trim().to_string()
        } else {
            errors.join("; ")
        };
        return Err(format!("go list failed: {detail}"));
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
        for f in &p.ignored_go_files {
            out.gaps.push(Gap::new("go", "build_constraint_excluded", Some(&file(f)),
                "file excluded by build constraints in the current environment; its imports are not analyzed"));
        }
    }
    Ok(out)
}
