//! Graph checks against hand-written fixtures. Expected edges and impact sets come from reading the
//! fixture sources, not from tool output. Toolchain-dependent cases report NOT_RUN when the tool is
//! missing, and fail instead when TAEGUK_REQUIRE_GRAPH_TOOLS=1.
use serde_json::Value;
use std::{
    collections::BTreeSet,
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

static SEQ: AtomicUsize = AtomicUsize::new(0);

struct Work(PathBuf);
impl Drop for Work {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
impl Work {
    fn repo(&self) -> PathBuf {
        self.0.join("repo")
    }
    fn graph(&self) -> PathBuf {
        self.0.join("graph.json")
    }
}

fn fixture_dir(lang: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/graph")
        .join(lang)
}

fn available(tool: &str, arg: &str) -> bool {
    let ok = Command::new(tool)
        .arg(arg)
        .output()
        .is_ok_and(|o| o.status.success());
    if !ok {
        assert!(
            env::var("TAEGUK_REQUIRE_GRAPH_TOOLS").as_deref() != Ok("1"),
            "{tool} is required but missing"
        );
        eprintln!("NOT_RUN: {tool} is not available");
    }
    ok
}

fn copy(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let name = e.file_name();
        if name.to_string_lossy().starts_with("expected-") {
            continue;
        }
        if e.file_type().unwrap().is_dir() {
            copy(&e.path(), &to.join(&name));
        } else {
            fs::copy(e.path(), to.join(&name)).unwrap();
        }
    }
}

fn git(dir: &Path, args: &[&str]) {
    let o = Command::new("git")
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.invalid",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        o.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&o.stderr)
    );
}

fn work(lang: &str) -> Work {
    let dir = env::temp_dir().join(format!(
        "taeguk-graph-test-{lang}-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::SeqCst)
    ));
    let w = Work(dir);
    copy(&fixture_dir(lang), &w.repo());
    // Build products stay untracked like in a real checkout.
    fs::write(
        w.repo().join(".gitignore"),
        "/_build/\n/target/\n/Cargo.lock\n",
    )
    .unwrap();
    git(&w.repo(), &["init", "-q"]);
    git(&w.repo(), &["add", "-A"]);
    git(&w.repo(), &["commit", "-q", "-m", "fixture"]);
    w
}

fn taeguk(args: &[&str]) -> (bool, Value) {
    let o = Command::new(env!("CARGO_BIN_EXE_taeguk"))
        .args(args)
        .output()
        .unwrap();
    let text = if o.status.success() {
        o.stdout
    } else {
        o.stderr
    };
    (o.status.success(), serde_json::from_slice(&text).unwrap())
}

fn build(w: &Work, lang: &str) -> Value {
    let (ok, v) = taeguk(&[
        "graph",
        "build",
        w.repo().to_str().unwrap(),
        "--out",
        w.graph().to_str().unwrap(),
        "--lang",
        lang,
    ]);
    assert!(ok, "{v}");
    serde_json::from_slice(&fs::read(w.graph()).unwrap()).unwrap()
}

fn expected(lang: &str, file: &str) -> BTreeSet<String> {
    fs::read_to_string(fixture_dir(lang).join(file))
        .unwrap()
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(Into::into)
        .collect()
}

fn check_fixture(lang: &str) {
    let w = work(lang);
    if lang == "elixir" {
        let o = Command::new("mix")
            .arg("compile")
            .current_dir(w.repo())
            .output()
            .unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    }
    let g = build(&w, lang);
    assert_eq!(g["status"], "PASS", "{g}");
    let edges: BTreeSet<String> = g["edges"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            format!(
                "{} {} {}",
                e["from"].as_str().unwrap(),
                e["to"].as_str().unwrap(),
                e["kind"].as_str().unwrap()
            )
        })
        .collect();
    assert_eq!(edges, expected(lang, "expected-edges.txt"));
    let gaps: BTreeSet<String> = g["gaps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            format!(
                "{} {} {}",
                e["lang"].as_str().unwrap(),
                e["kind"].as_str().unwrap(),
                e["file"].as_str().unwrap()
            )
        })
        .collect();
    assert_eq!(gaps, expected(lang, "expected-gaps.txt"));
}

#[test]
fn python_fixture_edges_and_gaps() {
    if available("python3", "--version") {
        check_fixture("python");
    }
}
#[test]
fn go_fixture_edges_and_gaps() {
    if available("go", "version") {
        check_fixture("go");
    }
}
#[test]
fn rust_fixture_edges_and_gaps() {
    if available("cargo", "--version") {
        check_fixture("rust");
    }
}
#[test]
fn elixir_fixture_edges_and_gaps() {
    if available("mix", "--version") {
        check_fixture("elixir");
    }
}

fn assert_fails(w: &Work, lang: &str, needle: &str) {
    let g = build(w, lang);
    assert_eq!(g["status"], "FAIL", "{g}");
    let a = &g["analyzers"][0];
    assert_eq!(a["status"], "FAIL");
    assert!(a["error"].as_str().unwrap().contains(needle), "{a}");
    assert_eq!(g["nodes"].as_array().unwrap().len(), 0);
}

#[test]
fn analyzer_failures_mark_the_graph_failed() {
    if available("python3", "--version") {
        let w = work("python");
        fs::write(w.repo().join("src/app/util.py"), "def broken(:\n").unwrap();
        git(&w.repo(), &["commit", "-qam", "break"]);
        assert_fails(&w, "python", "python parse errors");
    }
    if available("go", "version") {
        let w = work("go");
        fs::write(
            w.repo().join("b/b.go"),
            "package b\n\nimport \"example.com/missing/x\"\n\nvar _ = x.X\n",
        )
        .unwrap();
        git(&w.repo(), &["commit", "-qam", "break"]);
        assert_fails(&w, "go", "go list failed");
    }
    if available("cargo", "--version") {
        let w = work("rust");
        fs::remove_file(w.repo().join("core/src/store.rs")).unwrap();
        git(&w.repo(), &["commit", "-qam", "break"]);
        assert_fails(&w, "rust", "file for mod store not found");
    }
    if available("mix", "--version") {
        let w = work("elixir");
        assert_fails(&w, "elixir", "_build/dev not found");
    }
}

#[test]
fn rebuild_is_stable_and_dirty_trees_are_recorded() {
    if !available("python3", "--version") {
        return;
    }
    let w = work("python");
    let first = build(&w, "python");
    let second = build(&w, "python");
    assert_eq!(first["digest"], second["digest"]);
    assert_eq!(first["dirty"], false);
    let out = Command::new("python3").arg("--version").output().unwrap();
    let reported = String::from_utf8_lossy(&out.stdout).trim().to_string()
        + String::from_utf8_lossy(&out.stderr).trim();
    assert_eq!(first["analyzers"][0]["version"].as_str().unwrap(), reported);
    fs::write(
        w.repo().join("scripts/helper.py"),
        "def go(value):\n    return None\n",
    )
    .unwrap();
    let dirty = build(&w, "python");
    assert_eq!(dirty["dirty"], true);
    assert_eq!(
        dirty["dirty_paths"],
        serde_json::json!(["scripts/helper.py"])
    );
    assert_ne!(dirty["digest"], first["digest"]);
}

fn impact(w: &Work, extra: &[&str]) -> Value {
    let (graph, repo) = (w.graph(), w.repo());
    let mut args = vec![
        "graph",
        "impact",
        graph.to_str().unwrap(),
        repo.to_str().unwrap(),
    ];
    args.extend_from_slice(extra);
    let (ok, v) = taeguk(&args);
    assert!(ok, "{v}");
    v
}
fn distances(v: &Value) -> BTreeSet<(String, u64)> {
    v["impacted"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| {
            (
                i["id"].as_str().unwrap().to_string(),
                i["distance"].as_u64().unwrap(),
            )
        })
        .collect()
}
fn set(items: &[(&str, u64)]) -> BTreeSet<(String, u64)> {
    items.iter().map(|(i, d)| (i.to_string(), *d)).collect()
}

#[test]
fn impact_follows_reverse_edges_and_reports_limits() {
    if !available("python3", "--version") {
        return;
    }
    let w = work("python");
    build(&w, "python");
    let v = impact(&w, &["--changed", "src/app/util.py"]);
    assert_eq!(v["status"], "PASS", "{v}");
    assert_eq!(
        distances(&v),
        set(&[
            ("python:src/app/util.py", 0),
            ("python:src/app/core.py", 1),
            ("python:src/app/cli.py", 2),
            ("python:scripts/run.py", 3)
        ])
    );
    assert!(
        v["limits"]
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l.as_str().unwrap().contains("missing edge is not evidence"))
    );
    let shallow = impact(&w, &["--changed", "src/app/util.py", "--depth", "1"]);
    assert_eq!(shallow["depth_limit"], 1);
    assert_eq!(
        distances(&shallow),
        set(&[("python:src/app/util.py", 0), ("python:src/app/core.py", 1)])
    );

    let unmapped = impact(&w, &["--changed", "README.md"]);
    assert_eq!(unmapped["status"], "UNKNOWN");
    assert_eq!(unmapped["unmapped"], serde_json::json!(["README.md"]));

    fs::write(
        w.repo().join("scripts/helper.py"),
        "def go(value):\n    return None\n",
    )
    .unwrap();
    git(&w.repo(), &["commit", "-qam", "change"]);
    let stale = impact(&w, &["--git-diff", "HEAD~1"]);
    assert_eq!(stale["status"], "UNKNOWN");
    assert!(
        stale["reasons"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r == "graph_commit_differs_from_head")
    );
    assert_eq!(
        distances(&stale),
        set(&[
            ("python:scripts/helper.py", 0),
            ("python:scripts/run.py", 1),
            ("python:tests/test_helper.py", 1)
        ])
    );

    fs::write(
        w.repo().join("scripts/helper.py"),
        "def go(value):\n    return 1\n",
    )
    .unwrap();
    build(&w, "python");
    let dirty = impact(&w, &["--changed", "scripts/helper.py"]);
    assert!(
        dirty["reasons"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r == "graph_built_from_dirty_tree")
    );
    git(&w.repo(), &["checkout", "--", "."]);

    fs::write(w.repo().join("src/app/core.py"), "def broken(:\n").unwrap();
    git(&w.repo(), &["commit", "-qam", "break"]);
    build(&w, "python");
    let failed = impact(&w, &["--changed", "src/app/core.py"]);
    assert!(
        failed["reasons"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r == "graph_status_not_pass")
    );
}

#[test]
fn go_impact_includes_test_imports() {
    if !available("go", "version") {
        return;
    }
    let w = work("go");
    build(&w, "go");
    let v = impact(&w, &["--changed", "a/a.go"]);
    assert_eq!(v["status"], "PASS", "{v}");
    assert_eq!(
        distances(&v),
        set(&[
            ("go:example.com/fx/a", 0),
            ("go:example.com/fx/cmd", 1),
            ("go:example.com/fx/c", 1),
            ("go:example.com/fx/b", 2)
        ])
    );
}

#[test]
fn tampered_graph_is_rejected() {
    if !available("python3", "--version") {
        return;
    }
    let w = work("python");
    build(&w, "python");
    let text = fs::read_to_string(w.graph())
        .unwrap()
        .replacen("\"import\"", "\"other\"", 1);
    fs::write(w.graph(), text).unwrap();
    let (ok, v) = taeguk(&[
        "graph",
        "impact",
        w.graph().to_str().unwrap(),
        w.repo().to_str().unwrap(),
        "--changed",
        "common.py",
    ]);
    assert!(!ok);
    assert!(v["error"].as_str().unwrap().contains("digest"));
}
