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
    time::{Duration, UNIX_EPOCH},
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
        // A tracked link to a device or outside ROOT is refused instead of read.
        let w = work("python");
        #[cfg(unix)]
        std::os::unix::fs::symlink("/dev/zero", w.repo().join("src/app/zero.py")).unwrap();
        git(&w.repo(), &["add", "-A"]);
        git(&w.repo(), &["commit", "-qm", "link"]);
        assert_fails(&w, "python", "resolves outside ROOT");
        // A FIFO in place of a tracked file must fail instead of blocking on open.
        let w = work("python");
        let fifo = w.repo().join("src/app/util.py");
        fs::remove_file(&fifo).unwrap();
        assert!(
            Command::new("mkfifo")
                .arg(&fifo)
                .status()
                .unwrap()
                .success()
        );
        assert_fails(&w, "python", "not a regular file");
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
        // A missing import used only by tests fails too.
        let w = work("go");
        fs::write(
            w.repo().join("c/m_test.go"),
            "package c\n\nimport _ \"example.com/missing/x\"\n",
        )
        .unwrap();
        git(&w.repo(), &["add", "-A"]);
        git(&w.repo(), &["commit", "-qm", "break"]);
        assert_fails(&w, "go", "example.com/missing/x");
        // A tracked source linked to a file outside ROOT is refused before go reads it.
        let w = work("go");
        let outside = w.0.join("outside.go");
        fs::write(&outside, "package b\n\nfunc Outside() {}\n").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, w.repo().join("b/outside.go")).unwrap();
        git(&w.repo(), &["add", "-A"]);
        git(&w.repo(), &["commit", "-qm", "link"]);
        assert_fails(&w, "go", "resolves outside ROOT");
        // go opens every file in a package directory, tracked or not.
        let w = work("go");
        assert!(
            Command::new("mkfifo")
                .arg(w.repo().join("b/extra.go"))
                .status()
                .unwrap()
                .success()
        );
        assert_fails(&w, "go", "b/extra.go is not a regular file");
        // A local module manifest under testdata is read when go.mod replaces a module with it.
        let w = work("go");
        fs::create_dir_all(w.repo().join("testdata/evil")).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink("/dev/zero", w.repo().join("testdata/evil/go.mod")).unwrap();
        git(&w.repo(), &["add", "-A"]);
        git(&w.repo(), &["commit", "-qm", "link"]);
        assert_fails(&w, "go", "testdata/evil/go.mod resolves outside ROOT");
        // go list does not follow .gitignore, so ignored directories are checked too.
        let w = work("go");
        fs::write(w.repo().join(".gitignore"), "/generated/\n").unwrap();
        fs::create_dir_all(w.repo().join("generated")).unwrap();
        fs::write(w.repo().join("generated/g.go"), "package generated\n").unwrap();
        assert!(
            Command::new("mkfifo")
                .arg(w.repo().join("generated/extra.go"))
                .status()
                .unwrap()
                .success()
        );
        assert_fails(&w, "go", "generated/extra.go is not a regular file");
        // An ignored `target` is skipped only when no selected analyzer reads it; go does.
        let w = work("go");
        fs::create_dir_all(w.repo().join("target")).unwrap();
        assert!(
            Command::new("mkfifo")
                .arg(w.repo().join("target/blocked.go"))
                .status()
                .unwrap()
                .success()
        );
        assert_fails(&w, "go", "target/blocked.go is not a regular file");
        // An ignored link to a directory outside ROOT, where go walks, is refused.
        let w = work("go");
        fs::write(w.repo().join(".gitignore"), "/localdep\n").unwrap();
        fs::create_dir_all(w.0.join("outside")).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(w.0.join("outside"), w.repo().join("localdep")).unwrap();
        assert_fails(&w, "go", "localdep resolves outside ROOT");
    }
    if available("cargo", "--version") {
        let w = work("rust");
        fs::remove_file(w.repo().join("core/src/store.rs")).unwrap();
        git(&w.repo(), &["commit", "-qam", "break"]);
        assert_fails(&w, "rust", "file for mod store not found");
        // `#[path]` back to a file on the current module chain must fail, not recurse forever.
        let w = work("rust");
        let lib = w.repo().join("core/src/lib.rs");
        let text = fs::read_to_string(&lib).unwrap();
        fs::write(&lib, text + "#[path = \"lib.rs\"]\nmod again;\n").unwrap();
        git(&w.repo(), &["commit", "-qam", "cycle"]);
        assert_fails(&w, "rust", "module cycle");
        // Two `mod`s per level reusing the next file would visit 2^30 modules without a cap.
        let w = work("rust");
        let lib = w.repo().join("core/src/lib.rs");
        let text = fs::read_to_string(&lib).unwrap();
        fs::write(&lib, text + "#[path = \"deep/l0.rs\"]\nmod deep;\n").unwrap();
        fs::create_dir_all(w.repo().join("core/src/deep")).unwrap();
        for i in 0..30 {
            let next = format!("l{}.rs", i + 1);
            fs::write(
                w.repo().join(format!("core/src/deep/l{i}.rs")),
                format!("#[path = \"{next}\"]\nmod a;\n#[path = \"{next}\"]\nmod b;\n"),
            )
            .unwrap();
        }
        fs::write(w.repo().join("core/src/deep/l30.rs"), "").unwrap();
        git(&w.repo(), &["add", "-A"]);
        git(&w.repo(), &["commit", "-qm", "wide"]);
        assert_fails(&w, "rust", "module expansion exceeds");
        // A long chain of distinct files would exhaust the stack before the file count bound.
        let w = work("rust");
        let lib = w.repo().join("core/src/lib.rs");
        let text = fs::read_to_string(&lib).unwrap();
        fs::write(&lib, text + "#[path = \"chain/c0.rs\"]\nmod chain;\n").unwrap();
        fs::create_dir_all(w.repo().join("core/src/chain")).unwrap();
        for i in 0..80 {
            fs::write(
                w.repo().join(format!("core/src/chain/c{i}.rs")),
                format!("#[path = \"c{}.rs\"]\nmod next;\n", i + 1),
            )
            .unwrap();
        }
        fs::write(w.repo().join("core/src/chain/c80.rs"), "").unwrap();
        git(&w.repo(), &["add", "-A"]);
        git(&w.repo(), &["commit", "-qm", "chain"]);
        assert_fails(&w, "rust", "nested deeper than");
        // cargo reads member manifests itself; a device link must fail before it runs.
        let w = work("rust");
        fs::remove_file(w.repo().join("app/Cargo.toml")).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink("/dev/zero", w.repo().join("app/Cargo.toml")).unwrap();
        git(&w.repo(), &["add", "-A"]);
        git(&w.repo(), &["commit", "-qm", "link"]);
        assert_fails(&w, "rust", "app/Cargo.toml resolves outside ROOT");
        // A directory merely named `target` can hold workspace members cargo reads.
        let w = work("rust");
        let manifest = w.repo().join("Cargo.toml");
        let text = fs::read_to_string(&manifest)
            .unwrap()
            .replace("\"legacy\"]", "\"legacy\", \"target/member\"]");
        fs::write(&manifest, text).unwrap();
        git(&w.repo(), &["commit", "-qam", "member"]);
        fs::create_dir_all(w.repo().join("target/member")).unwrap();
        assert!(
            Command::new("mkfifo")
                .arg(w.repo().join("target/member/Cargo.toml"))
                .status()
                .unwrap()
                .success()
        );
        assert_fails(&w, "rust", "target/member/Cargo.toml is not a regular file");
    }
    if available("mix", "--version") {
        // mix.exs is read before anything else, so a device link must fail without a build.
        let w = work("elixir");
        fs::remove_file(w.repo().join("mix.exs")).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink("/dev/zero", w.repo().join("mix.exs")).unwrap();
        git(&w.repo(), &["add", "-A"]);
        git(&w.repo(), &["commit", "-qm", "link"]);
        assert_fails(&w, "elixir", "mix.exs resolves outside ROOT");
        let w = work("elixir");
        assert_fails(&w, "elixir", "_build/dev not found");
        // A source newer than the build must fail instead of being compiled by xref.
        let o = Command::new("mix")
            .arg("compile")
            .current_dir(w.repo())
            .output()
            .unwrap();
        assert!(o.status.success());
        let manifest = w.repo().join("_build/dev/lib/fx/.mix/compile.elixir");
        let before = fs::metadata(&manifest).unwrap().modified().unwrap();
        // Mix stores the manifest time in whole seconds; a source saved within that second is not newer.
        let second = before.duration_since(UNIX_EPOCH).unwrap().as_secs();
        fs::File::options()
            .write(true)
            .open(w.repo().join("lib/fx/a.ex"))
            .unwrap()
            .set_modified(UNIX_EPOCH + Duration::from_millis(second * 1000 + 900))
            .unwrap();
        assert_eq!(build(&w, "elixir")["status"], "PASS");
        // An edit after the compile (same second or not) is seen through the content digest.
        let a = w.repo().join("lib/fx/a.ex");
        fs::write(&a, fs::read_to_string(&a).unwrap() + "\n# edited\n").unwrap();
        assert_fails(&w, "elixir", "changed since the last compile");
        assert_eq!(fs::metadata(&manifest).unwrap().modified().unwrap(), before);
        // A source deleted after the last compile is still in the manifest, so the build is stale.
        let w = work("elixir");
        let o = Command::new("mix")
            .arg("compile")
            .current_dir(w.repo())
            .output()
            .unwrap();
        assert!(o.status.success());
        git(&w.repo(), &["rm", "-q", "lib/fx/dyn.ex"]);
        git(&w.repo(), &["commit", "-qm", "delete"]);
        assert_fails(&w, "elixir", "no longer tracked");
        // Only the compile manifest of the ignored build is read; a FIFO there fails instead of
        // blocking.
        let w = work("elixir");
        let o = Command::new("mix")
            .arg("compile")
            .current_dir(w.repo())
            .output()
            .unwrap();
        assert!(o.status.success());
        let manifest = w.repo().join("_build/dev/lib/fx/.mix/compile.elixir");
        fs::remove_file(&manifest).unwrap();
        assert!(
            Command::new("mkfifo")
                .arg(&manifest)
                .status()
                .unwrap()
                .success()
        );
        assert_fails(&w, "elixir", "must be a regular file inside ROOT");
        // The build directory itself must not be a link out of ROOT.
        let w = work("elixir");
        let o = Command::new("mix")
            .arg("compile")
            .current_dir(w.repo())
            .output()
            .unwrap();
        assert!(o.status.success());
        let outside = w.0.join("outside-build");
        fs::rename(w.repo().join("_build/dev"), &outside).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, w.repo().join("_build/dev")).unwrap();
        assert_fails(&w, "elixir", "must be a regular file inside ROOT");
    }
}

#[test]
fn output_links_are_not_followed() {
    if !available("python3", "--version") {
        return;
    }
    let w = work("python");
    let victim = w.0.join("victim.txt");
    fs::write(&victim, "keep\n").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&victim, w.repo().join("graph.json")).unwrap();
    let out = w.repo().join("graph.json");
    let (ok, v) = taeguk(&[
        "graph",
        "build",
        w.repo().to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);
    assert!(!ok, "{v}");
    assert!(v["error"].as_str().unwrap().contains("is a link"), "{v}");
    assert_eq!(fs::read_to_string(&victim).unwrap(), "keep\n");
    // A linked directory inside ROOT that leads outside is refused as well.
    fs::remove_file(&out).unwrap();
    fs::create_dir_all(w.0.join("elsewhere")).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(w.0.join("elsewhere"), w.repo().join("outdir")).unwrap();
    let out = w.repo().join("outdir/graph.json");
    let (ok, v) = taeguk(&[
        "graph",
        "build",
        w.repo().to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);
    assert!(!ok, "{v}");
    assert!(v["error"].as_str().unwrap().contains("leaves ROOT"), "{v}");
    assert!(!w.0.join("elsewhere/graph.json").exists());
    // An existing FIFO is replaced by the new file instead of being opened.
    let out = w.0.join("fifo.json");
    assert!(Command::new("mkfifo").arg(&out).status().unwrap().success());
    let (ok, v) = taeguk(&[
        "graph",
        "build",
        w.repo().to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);
    assert!(ok, "{v}");
    assert!(fs::symlink_metadata(&out).unwrap().is_file());
}

#[test]
fn ignored_inputs_make_impact_unknown() {
    if !available("go", "version") {
        return;
    }
    // go list reads ignored sources, so a change there must not leave an old graph current.
    let w = work("go");
    fs::write(w.repo().join(".gitignore"), "/a/generated.go\n").unwrap();
    git(&w.repo(), &["commit", "-qam", "ignore generated"]);
    build(&w, "go");
    let v = impact(&w, &["--changed", "b/b.go"]);
    assert_eq!(other_reasons(&v), Vec::<String>::new());
    fs::write(
        w.repo().join("a/generated.go"),
        "package a\n\nimport _ \"example.com/fx/b\"\n",
    )
    .unwrap();
    let v = impact(&w, &["--changed", "b/b.go"]);
    assert_eq!(v["status"], "UNKNOWN");
    assert_eq!(other_reasons(&v), ["analyzer_inputs_changed"]);
}

#[test]
fn ignored_rust_members_are_fingerprinted() {
    if !available("cargo", "--version") {
        return;
    }
    // A workspace member under an ignored directory named `target` is read by cargo.
    let w = work("rust");
    let manifest = w.repo().join("Cargo.toml");
    let text = fs::read_to_string(&manifest)
        .unwrap()
        .replace("\"legacy\"]", "\"legacy\", \"target/member\"]");
    fs::write(&manifest, text).unwrap();
    git(&w.repo(), &["commit", "-qam", "member"]);
    let member = w.repo().join("target/member");
    fs::create_dir_all(member.join("src")).unwrap();
    fs::write(
        member.join("Cargo.toml"),
        "[package]\nname = \"member\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    fs::write(member.join("src/lib.rs"), "pub fn m() {}\n").unwrap();
    let g = build(&w, "rust");
    assert_eq!(g["status"], "PASS", "{g}");
    let v = impact(&w, &["--changed", "core/src/store.rs"]);
    assert_eq!(other_reasons(&v), Vec::<String>::new());
    fs::write(member.join("src/lib.rs"), "pub fn m() {}\npub fn n() {}\n").unwrap();
    let v = impact(&w, &["--changed", "core/src/store.rs"]);
    assert_eq!(other_reasons(&v), ["analyzer_inputs_changed"]);
}

#[test]
fn analyzers_run_isolated_with_a_time_limit() {
    if !available("cargo", "--version") {
        return;
    }
    // A parser crash on hostile input is that analyzer's FAIL, not a crashed CLI.
    let w = work("rust");
    let deep = "mod a {\n".repeat(200_000) + &"}\n".repeat(200_000);
    fs::write(w.repo().join("core/src/deep.rs"), deep).unwrap();
    let lib = w.repo().join("core/src/lib.rs");
    fs::write(&lib, fs::read_to_string(&lib).unwrap() + "mod deep;\n").unwrap();
    git(&w.repo(), &["add", "-A"]);
    git(&w.repo(), &["commit", "-qm", "deep"]);
    let g = build(&w, "rust");
    assert_eq!(g["status"], "FAIL", "{g}");
    assert_eq!(g["nodes"].as_array().unwrap().len(), 0);
    // A wait that never ends (a FIFO manifest cargo reads through an ignored link the scan
    // allows) is stopped at the time limit.
    let w = work("rust");
    let outside = w.0.join("outside/m");
    fs::create_dir_all(&outside).unwrap();
    assert!(
        Command::new("mkfifo")
            .arg(outside.join("Cargo.toml"))
            .status()
            .unwrap()
            .success()
    );
    fs::write(w.repo().join(".gitignore"), "/.linked\n").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(w.0.join("outside"), w.repo().join(".linked")).unwrap();
    let manifest = w.repo().join("Cargo.toml");
    let text = fs::read_to_string(&manifest)
        .unwrap()
        .replace("\"legacy\"]", "\"legacy\", \".linked/m\"]");
    fs::write(&manifest, text).unwrap();
    git(&w.repo(), &["commit", "-qam", "linked"]);
    let o = Command::new(env!("CARGO_BIN_EXE_taeguk"))
        .args(["graph", "build"])
        .arg(w.repo())
        .arg("--out")
        .arg(w.graph())
        .args(["--lang", "rust"])
        .env("TAEGUK_GRAPH_TIMEOUT_SECS", "3")
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let g: Value = serde_json::from_slice(&fs::read(w.graph()).unwrap()).unwrap();
    assert_eq!(g["status"], "FAIL", "{g}");
    assert!(
        g["analyzers"][0]["error"]
            .as_str()
            .unwrap()
            .contains("did not finish within 3 s"),
        "{g}"
    );
    // The analyzer was stopped; the cargo it started must not be left running.
    std::thread::sleep(Duration::from_millis(500));
    let left = Command::new("pgrep")
        .args(["-f", w.repo().to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        left.stdout.is_empty(),
        "{}",
        String::from_utf8_lossy(&left.stdout)
    );
}

#[test]
fn standard_macro_names_redefined_in_the_crate_are_gaps() {
    if !available("cargo", "--version") {
        return;
    }
    let w = work("rust");
    fs::write(
        w.repo().join("core/src/shadow.rs"),
        "macro_rules! vec {\n    () => {\n        0\n    };\n}\n\npub fn f() -> i32 {\n    vec!()\n}\n",
    )
    .unwrap();
    let lib = w.repo().join("core/src/lib.rs");
    fs::write(
        &lib,
        fs::read_to_string(&lib).unwrap() + "pub mod shadow;\n",
    )
    .unwrap();
    git(&w.repo(), &["add", "-A"]);
    git(&w.repo(), &["commit", "-qm", "shadow"]);
    let g = build(&w, "rust");
    assert!(
        g["gaps"]
            .as_array()
            .unwrap()
            .iter()
            .any(|x| x["file"] == "core/src/shadow.rs"
                && x["detail"]
                    .as_str()
                    .unwrap()
                    .contains("vec! may be redefined")),
        "{g}"
    );
}

#[test]
fn mix_exs_is_never_executed() {
    if !available("mix", "--version") {
        return;
    }
    // mix.exs is project code; building the graph reads the manifest and must not run it.
    let w = work("elixir");
    let marker = w.0.join("executed");
    let mix = w.repo().join("mix.exs");
    let text = format!(
        "File.write!({:?}, \"x\")\n{}",
        marker.to_str().unwrap(),
        fs::read_to_string(&mix).unwrap()
    );
    fs::write(&mix, text).unwrap();
    git(&w.repo(), &["commit", "-qam", "marker"]);
    let o = Command::new("mix")
        .arg("compile")
        .current_dir(w.repo())
        .output()
        .unwrap();
    assert!(o.status.success());
    assert!(marker.exists(), "mix compile itself runs mix.exs");
    fs::remove_file(&marker).unwrap();
    let g = build(&w, "elixir");
    assert_eq!(g["status"], "PASS", "{g}");
    assert!(!marker.exists());
}

#[test]
fn toolchain_files_in_the_repository_are_not_used() {
    if !available("cargo", "--version") {
        return;
    }
    // rust-toolchain.toml can name a toolchain path, i.e. a program in the repository.
    let w = work("rust");
    let marker = w.0.join("executed");
    let bin = w.repo().join("evil/bin");
    fs::create_dir_all(&bin).unwrap();
    for tool in ["cargo", "rustc"] {
        fs::write(
            bin.join(tool),
            format!("#!/bin/sh\ntouch {}\nexit 1\n", marker.to_str().unwrap()),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(bin.join(tool), fs::Permissions::from_mode(0o755)).unwrap();
        }
    }
    fs::write(
        w.repo().join("rust-toolchain.toml"),
        format!(
            "[toolchain]\npath = {:?}\n",
            w.repo().join("evil").to_str().unwrap()
        ),
    )
    .unwrap();
    git(&w.repo(), &["add", "-A"]);
    git(&w.repo(), &["commit", "-qm", "toolchain"]);
    let g = build(&w, "rust");
    assert_eq!(g["status"], "PASS", "{g}");
    impact(&w, &["--changed", "core/src/store.rs"]);
    assert!(!marker.exists(), "a program named by the repository ran");
    // A failed scan runs no tool at all, so a FIFO toolchain file cannot block a version query.
    let w = work("rust");
    assert!(
        Command::new("mkfifo")
            .arg(w.repo().join("rust-toolchain.toml"))
            .status()
            .unwrap()
            .success()
    );
    let g = build(&w, "rust");
    assert_eq!(g["status"], "FAIL", "{g}");
    assert!(g["analyzers"][0]["version"].is_null());
}

#[test]
fn hostile_manifests_and_git_config_run_nothing() {
    // git configuration in the checkout (fsmonitor, filter drivers) must not run programs.
    if available("python3", "--version") {
        let w = work("python");
        let marker = w.0.join("git-ran");
        let hook = format!("touch {}", marker.to_str().unwrap());
        git(&w.repo(), &["config", "core.fsmonitor", &hook]);
        git(&w.repo(), &["config", "filter.evil.clean", &hook]);
        fs::write(w.repo().join(".gitattributes"), "*.py filter=evil\n").unwrap();
        fs::write(w.repo().join("common.py"), "VALUE = 2\n").unwrap();
        build(&w, "python");
        impact(&w, &["--changed", "common.py"]);
        assert!(
            !marker.exists(),
            "repository git configuration ran a program"
        );
        // Attributes from core.attributesFile and .git/info/attributes cannot run filters either.
        let attrs = w.0.join("attrs");
        fs::write(&attrs, "*.py filter=evil\n").unwrap();
        git(
            &w.repo(),
            &["config", "core.attributesFile", attrs.to_str().unwrap()],
        );
        fs::create_dir_all(w.repo().join(".git/info")).unwrap();
        fs::write(w.repo().join(".git/info/attributes"), "*.py filter=evil\n").unwrap();
        fs::write(w.repo().join("common.py"), "VALUE = 3\n").unwrap();
        build(&w, "python");
        impact(&w, &["--git-diff", "HEAD"]);
        assert!(
            !marker.exists(),
            "an attribute file outside the tree ran a filter"
        );
    }
    // A manifest is data; one carrying a Stream with a callback is refused, not enumerated.
    if available("mix", "--version") {
        let w = work("elixir");
        let o = Command::new("mix")
            .arg("compile")
            .current_dir(w.repo())
            .output()
            .unwrap();
        assert!(o.status.success());
        let marker = w.0.join("manifest-ran");
        let manifest = w.repo().join("_build/dev/lib/fx/.mix/compile.elixir");
        let craft = format!(
            "t = :erlang.binary_to_term(File.read!({m:?})); \
             s = Stream.map([{c:?}], fn c -> System.cmd(\"touch\", [c]) end); \
             File.write!({m:?}, :erlang.term_to_binary(put_elem(t, 2, s)))",
            m = manifest.to_str().unwrap(),
            c = marker.to_str().unwrap()
        );
        let o = Command::new("elixir")
            .args(["-e", &craft])
            .output()
            .unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        assert_fails(&w, "elixir", "not plain data");
        assert!(!marker.exists(), "manifest content ran");
        // A source named like an option (`-r`) is data on stdin, never an elixir argument.
        let w = work("elixir");
        let o = Command::new("mix")
            .arg("compile")
            .current_dir(w.repo())
            .output()
            .unwrap();
        assert!(o.status.success());
        let marker = w.0.join("option-ran");
        fs::write(
            w.repo().join("-r"),
            format!("File.write!({:?}, \"x\")\n", marker.to_str().unwrap()),
        )
        .unwrap();
        git(&w.repo(), &["add", "--", "-r"]);
        git(&w.repo(), &["commit", "-qm", "dash"]);
        let manifest = w.repo().join("_build/dev/lib/fx/.mix/compile.elixir");
        let craft = format!(
            "t = :erlang.binary_to_term(File.read!({m:?})); s = elem(t, 2); \
             File.write!({m:?}, :erlang.term_to_binary(put_elem(t, 2, Map.put(s, \"-r\", s[\"lib/fx/a.ex\"]))))",
            m = manifest.to_str().unwrap()
        );
        let o = Command::new("elixir")
            .args(["-e", &craft])
            .output()
            .unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        build(&w, "elixir");
        assert!(
            !marker.exists(),
            "a repository file name was read as an elixir option"
        );
    }
}

#[test]
fn manifest_link_retargeting_changes_the_fingerprint() {
    if !available("mix", "--version") {
        return;
    }
    // The manifest read through a link: repointing the link must make the graph stale.
    let w = work("elixir");
    let o = Command::new("mix")
        .arg("compile")
        .current_dir(w.repo())
        .output()
        .unwrap();
    assert!(o.status.success());
    let dir = w.repo().join("_build/dev/lib/fx/.mix");
    fs::rename(dir.join("compile.elixir"), dir.join("a.manifest")).unwrap();
    let mut other = fs::read(dir.join("a.manifest")).unwrap();
    other.push(0);
    fs::write(dir.join("b.manifest"), other).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink("a.manifest", dir.join("compile.elixir")).unwrap();
    let g = build(&w, "elixir");
    assert_eq!(g["status"], "PASS", "{g}");
    let v = impact(&w, &["--changed", "lib/fx/a.ex"]);
    assert_eq!(other_reasons(&v), Vec::<String>::new());
    fs::remove_file(dir.join("compile.elixir")).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink("b.manifest", dir.join("compile.elixir")).unwrap();
    let v = impact(&w, &["--changed", "lib/fx/a.ex"]);
    assert_eq!(other_reasons(&v), ["analyzer_inputs_changed"]);
}

#[test]
fn python_resolve_follows_links_inside_root() {
    if !available("python3", "--version") {
        return;
    }
    // `Path(__file__).resolve()` of a linked script is the link target's directory.
    let w = work("python");
    fs::create_dir_all(w.repo().join("lnk")).unwrap();
    fs::write(
        w.repo().join("lnk/real.py"),
        "import sys\nfrom pathlib import Path\n\nsys.path.insert(0, str(Path(__file__).resolve().parent))\n\nimport sibling  # noqa: E402\n",
    )
    .unwrap();
    fs::write(w.repo().join("lnk/sibling.py"), "VALUE = 1\n").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink("../lnk/real.py", w.repo().join("scripts/linked.py")).unwrap();
    git(&w.repo(), &["add", "-A"]);
    git(&w.repo(), &["commit", "-qm", "link"]);
    let g = build(&w, "python");
    let edges: BTreeSet<String> = g["edges"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            format!(
                "{} {}",
                e["from"].as_str().unwrap(),
                e["to"].as_str().unwrap()
            )
        })
        .collect();
    assert!(
        edges.contains("python:scripts/linked.py python:lnk/sibling.py"),
        "{edges:?}"
    );
    // A tracked link is compared by its target text: unchanged, it is not dirty.
    assert_eq!(g["dirty"], false, "{g}");
    fs::remove_file(w.repo().join("scripts/linked.py")).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink("../lnk/sibling.py", w.repo().join("scripts/linked.py")).unwrap();
    let g = build(&w, "python");
    assert_eq!(
        g["dirty_paths"],
        serde_json::json!(["scripts/linked.py"]),
        "{g}"
    );
}

#[test]
fn files_analyzers_read_are_fingerprinted() {
    if available("python3", "--version") {
        // An ignored pyproject.toml still sets the package roots.
        let w = work("python");
        git(&w.repo(), &["rm", "-q", "--cached", "pyproject.toml"]);
        fs::write(w.repo().join(".gitignore"), "/pyproject.toml\n").unwrap();
        git(&w.repo(), &["add", "-A"]);
        git(&w.repo(), &["commit", "-qm", "ignore pyproject"]);
        build(&w, "python");
        let v = impact(&w, &["--changed", "common.py"]);
        assert_eq!(other_reasons(&v), Vec::<String>::new());
        let p = w.repo().join("pyproject.toml");
        fs::write(&p, fs::read_to_string(&p).unwrap() + "# changed\n").unwrap();
        let v = impact(&w, &["--changed", "common.py"]);
        assert_eq!(other_reasons(&v), ["analyzer_inputs_changed"]);
    }
    if available("python3", "--version") {
        // An absent pyproject.toml is recorded too, so adding one later is noticed.
        let w = work("python");
        git(&w.repo(), &["rm", "-q", "pyproject.toml"]);
        fs::write(w.repo().join(".gitignore"), "/pyproject.toml\n").unwrap();
        git(&w.repo(), &["add", "-A"]);
        git(&w.repo(), &["commit", "-qm", "no pyproject"]);
        build(&w, "python");
        let v = impact(&w, &["--changed", "common.py"]);
        assert_eq!(other_reasons(&v), Vec::<String>::new());
        fs::write(
            w.repo().join("pyproject.toml"),
            "[tool.setuptools.packages.find]\nwhere = [\"src\"]\n",
        )
        .unwrap();
        let v = impact(&w, &["--changed", "common.py"]);
        assert_eq!(other_reasons(&v), ["analyzer_inputs_changed"]);
    }
    if available("cargo", "--version") {
        // A `#[path]` module file with another extension, ignored by git, is read too.
        let w = work("rust");
        fs::write(
            w.repo().join(".gitignore"),
            "/target/\n/Cargo.lock\n/core/src/gen.inc\n",
        )
        .unwrap();
        fs::write(w.repo().join("core/src/gen.inc"), "pub fn g() {}\n").unwrap();
        let lib = w.repo().join("core/src/lib.rs");
        let text = fs::read_to_string(&lib).unwrap() + "#[path = \"gen.inc\"]\nmod generated;\n";
        fs::write(&lib, text).unwrap();
        git(&w.repo(), &["add", "-A"]);
        git(&w.repo(), &["commit", "-qm", "gen"]);
        build(&w, "rust");
        let v = impact(&w, &["--changed", "core/src/store.rs"]);
        assert_eq!(other_reasons(&v), Vec::<String>::new());
        fs::write(w.repo().join("core/src/gen.inc"), "pub fn h() {}\n").unwrap();
        let v = impact(&w, &["--changed", "core/src/store.rs"]);
        assert_eq!(other_reasons(&v), ["analyzer_inputs_changed"]);
    }
}

#[test]
fn go_embed_inputs_are_fingerprinted() {
    if !available("go", "version") {
        return;
    }
    // An embedded file is an input even when git ignores it and its extension is not Go's.
    let w = work("go");
    fs::write(w.repo().join(".gitignore"), "/a/payload.txt\n").unwrap();
    fs::write(
        w.repo().join("a/embed.go"),
        "package a\n\nimport _ \"embed\"\n\n//go:embed payload.txt\nvar Payload string\n",
    )
    .unwrap();
    git(&w.repo(), &["add", "-A"]);
    git(&w.repo(), &["commit", "-qm", "embed"]);
    fs::write(w.repo().join("a/payload.txt"), "data\n").unwrap();
    let g = build(&w, "go");
    assert_eq!(g["status"], "PASS", "{g}");
    let v = impact(&w, &["--changed", "b/b.go"]);
    assert_eq!(other_reasons(&v), Vec::<String>::new());
    fs::remove_file(w.repo().join("a/payload.txt")).unwrap();
    let v = impact(&w, &["--changed", "b/b.go"]);
    assert_eq!(other_reasons(&v), ["analyzer_inputs_changed"]);
}

#[test]
fn go_embed_of_build_output_is_fingerprinted() {
    if !available("go", "version") {
        return;
    }
    // `all:_build` embeds a directory the ROOT walk skips as build output.
    let w = work("go");
    fs::write(w.repo().join(".gitignore"), "/a/_build/\n").unwrap();
    fs::write(
        w.repo().join("a/embed.go"),
        "package a\n\nimport \"embed\"\n\n//go:embed all:_build\nvar Files embed.FS\n",
    )
    .unwrap();
    git(&w.repo(), &["add", "-A"]);
    git(&w.repo(), &["commit", "-qm", "embed"]);
    fs::create_dir_all(w.repo().join("a/_build/dev/lib")).unwrap();
    fs::write(w.repo().join("a/_build/dev/lib/data.txt"), "one\n").unwrap();
    let g = build(&w, "go");
    assert_eq!(g["status"], "PASS", "{g}");
    fs::write(w.repo().join("a/_build/dev/lib/data.txt"), "two\n").unwrap();
    let v = impact(&w, &["--changed", "b/b.go"]);
    assert_eq!(other_reasons(&v), ["analyzer_inputs_changed"]);
    // A failing import read from an ignored `.go` link out of ROOT is not named in the error.
    let w = work("go");
    let outside = w.0.join("outside.go");
    fs::write(
        &outside,
        "package b\n\nimport _ \"corp.example/private/payment\"\n",
    )
    .unwrap();
    fs::write(w.repo().join(".gitignore"), "/b/extra.go\n").unwrap();
    git(&w.repo(), &["add", "-A"]);
    git(&w.repo(), &["commit", "-qm", "ignore"]);
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, w.repo().join("b/extra.go")).unwrap();
    let (ok, v) = taeguk(&[
        "graph",
        "build",
        w.repo().to_str().unwrap(),
        "--out",
        w.graph().to_str().unwrap(),
        "--lang",
        "go",
    ]);
    assert!(ok, "{v}");
    let graph = fs::read_to_string(w.graph()).unwrap();
    assert!(!graph.contains("corp.example"), "{graph}");
}

#[test]
fn go_embed_at_module_root_can_be_queried() {
    if !available("go", "version") {
        return;
    }
    // A package at the module root embeds from the whole ROOT tree; the graph build writes for
    // it must be one impact accepts, and a change to the embedded file makes it stale.
    let w = work("go");
    fs::write(
        w.repo().join("root.go"),
        "package fx\n\nimport _ \"embed\"\n\n//go:embed payload.txt\nvar Payload string\n",
    )
    .unwrap();
    fs::write(w.repo().join("payload.txt"), "one\n").unwrap();
    fs::write(w.repo().join(".gitignore"), "/payload.txt\n").unwrap();
    git(&w.repo(), &["add", "-A"]);
    git(&w.repo(), &["commit", "-qm", "root embed"]);
    let g = build(&w, "go");
    assert_eq!(g["status"], "PASS", "{g}");
    assert!(
        g["read"].as_array().unwrap().iter().any(|r| r == "./"),
        "{g}"
    );
    let v = impact(&w, &["--changed", "b/b.go"]);
    assert!(other_reasons(&v).is_empty(), "{v}");
    fs::write(w.repo().join("payload.txt"), "two\n").unwrap();
    let v = impact(&w, &["--changed", "b/b.go"]);
    assert_eq!(other_reasons(&v), ["analyzer_inputs_changed"]);
}

#[test]
fn rust_path_attribute_walks_links_like_the_os() {
    if !available("cargo", "--version") {
        return;
    }
    // `lnk/..` after a link to `real/deep` is `real`, as the kernel walks it, not `.`.
    let w = work("rust");
    let src = w.repo().join("app/src");
    fs::create_dir_all(src.join("real/deep")).unwrap();
    fs::write(
        src.join("real/choice.rs"),
        "use fx_core::model::Item;\npub fn take(_: Item) {}\n",
    )
    .unwrap();
    fs::write(src.join("choice.rs"), "pub fn take() {}\n").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink("real/deep", src.join("lnk")).unwrap();
    let main = fs::read_to_string(src.join("main.rs")).unwrap();
    fs::write(
        src.join("main.rs"),
        format!("#[path = \"lnk/../choice.rs\"]\nmod choice;\n{main}"),
    )
    .unwrap();
    git(&w.repo(), &["add", "-A"]);
    git(&w.repo(), &["commit", "-qm", "path link"]);
    let g = build(&w, "rust");
    assert_eq!(g["status"], "PASS", "{g}");
    let ids: Vec<&str> = g["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["id"].as_str().unwrap())
        .collect();
    assert!(ids.contains(&"rust:app/src/real/choice.rs"), "{ids:?}");
    assert!(!ids.contains(&"rust:app/src/choice.rs"), "{ids:?}");
    assert!(
        g["edges"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["from"] == "rust:app/src/real/choice.rs"
                && e["to"] == "rust:core/src/model.rs"),
        "{g}"
    );
}

#[test]
fn go_configuration_outside_root_is_not_read() {
    if !available("go", "version") {
        return;
    }
    // An ignored go.mod linked to another project's manifest: its module path must not reach
    // the graph, and the graph is FAIL.
    let w = work("go");
    let outside = w.0.join("outside.mod");
    fs::write(&outside, "module corp.example/secret\n\ngo 1.22\n").unwrap();
    git(&w.repo(), &["rm", "-q", "--cached", "go.mod"]);
    fs::write(w.repo().join(".gitignore"), "/go.mod\n").unwrap();
    git(&w.repo(), &["add", "-A"]);
    git(&w.repo(), &["commit", "-qm", "ignore go.mod"]);
    fs::remove_file(w.repo().join("go.mod")).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, w.repo().join("go.mod")).unwrap();
    let (ok, v) = taeguk(&[
        "graph",
        "build",
        w.repo().to_str().unwrap(),
        "--out",
        w.graph().to_str().unwrap(),
        "--lang",
        "go",
    ]);
    assert!(ok, "{v}");
    let graph = fs::read_to_string(w.graph()).unwrap();
    assert!(!graph.contains("corp.example"), "{graph}");
    let g: Value = serde_json::from_str(&graph).unwrap();
    assert_eq!(g["status"], "FAIL", "{g}");
    // A go.work inside ROOT may add a module outside it: the packages only that module's
    // sources name must not appear in the failure message.
    let w = work("go");
    let private = w.0.join("private");
    fs::create_dir_all(private.join("p")).unwrap();
    fs::create_dir_all(private.join("internal/secretpkg")).unwrap();
    fs::write(
        private.join("go.mod"),
        "module corp.example/private\n\ngo 1.22\n",
    )
    .unwrap();
    fs::write(
        private.join("p/p.go"),
        "package p\n\nimport _ \"corp.example/private/internal/secretpkg\"\n",
    )
    .unwrap();
    fs::write(
        private.join("internal/secretpkg/s.go"),
        "package secretpkg\n",
    )
    .unwrap();
    fs::write(
        w.repo().join("go.work"),
        "go 1.22\n\nuse .\nuse ../private\n",
    )
    .unwrap();
    fs::write(
        w.repo().join("a/uses.go"),
        "package a\n\nimport _ \"corp.example/private/p\"\n",
    )
    .unwrap();
    git(&w.repo(), &["add", "-A"]);
    git(&w.repo(), &["commit", "-qm", "workspace"]);
    let (ok, v) = taeguk(&[
        "graph",
        "build",
        w.repo().to_str().unwrap(),
        "--out",
        w.graph().to_str().unwrap(),
        "--lang",
        "go",
    ]);
    assert!(ok, "{v}");
    let graph = fs::read_to_string(w.graph()).unwrap();
    assert!(!graph.contains("secretpkg"), "{graph}");
    let g: Value = serde_json::from_str(&graph).unwrap();
    assert_eq!(g["status"], "FAIL", "{g}");
    // A go.work above ROOT is not used (go would otherwise search parent directories for it).
    let w = work("go");
    fs::write(w.0.join("go.work"), "this is not a go.work file\n").unwrap();
    let g = build(&w, "go");
    assert_eq!(g["status"], "PASS", "{g}");
}

#[test]
fn git_waiting_on_a_fifo_is_bounded() {
    if !available("python3", "--version") {
        return;
    }
    // git opens `.gitignore` with a blocking open; a FIFO there must not hang build or impact.
    let w = work("python");
    build(&w, "python");
    fs::remove_file(w.repo().join(".gitignore")).ok();
    assert!(
        Command::new("mkfifo")
            .arg(w.repo().join(".gitignore"))
            .status()
            .unwrap()
            .success()
    );
    let started = std::time::Instant::now();
    for args in [
        vec![
            "graph",
            "build",
            w.repo().to_str().unwrap(),
            "--out",
            w.0.join("other.json").to_str().unwrap(),
            "--lang",
            "python",
        ],
        vec![
            "graph",
            "impact",
            w.graph().to_str().unwrap(),
            w.repo().to_str().unwrap(),
            "--changed",
            "common.py",
        ],
    ] {
        let o = Command::new(env!("CARGO_BIN_EXE_taeguk"))
            .args(&args)
            .env("TAEGUK_GRAPH_TIMEOUT_SECS", "2")
            .output()
            .unwrap();
        let text =
            String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr);
        assert!(
            text.contains("did not finish within 2 s"),
            "{args:?}: {text}"
        );
    }
    assert!(started.elapsed() < Duration::from_secs(30));
}

#[test]
fn python_path_links_are_fingerprinted() {
    if !available("python3", "--version") {
        return;
    }
    // An ignored link a `sys.path` entry goes through decides which `helper` is imported;
    // retargeting it must make the graph stale even though no file content changed.
    let w = work("python");
    for d in ["ta", "tb", "tr"] {
        fs::create_dir_all(w.repo().join(d)).unwrap();
    }
    fs::write(w.repo().join("ta/helper.py"), "X = 1\n").unwrap();
    fs::write(w.repo().join("tb/helper.py"), "X = 1\n").unwrap();
    fs::write(
        w.repo().join("tr/run.py"),
        "import sys\nfrom pathlib import Path\n\nsys.path.insert(0, str(Path(__file__).parent.parent / \"alias\"))\nimport helper\n",
    )
    .unwrap();
    fs::write(w.repo().join(".gitignore"), "/alias\n").unwrap();
    git(&w.repo(), &["add", "-A"]);
    git(&w.repo(), &["commit", "-qm", "alias"]);
    #[cfg(unix)]
    std::os::unix::fs::symlink("ta", w.repo().join("alias")).unwrap();
    let g = build(&w, "python");
    assert_eq!(g["status"], "PASS", "{g}");
    let edges: Vec<_> = g["edges"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["from"] == "python:tr/run.py")
        .map(|e| e["to"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(edges, ["python:ta/helper.py"]);
    fs::remove_file(w.repo().join("alias")).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink("tb", w.repo().join("alias")).unwrap();
    let v = impact(&w, &["--changed", "common.py"]);
    assert_eq!(other_reasons(&v), ["analyzer_inputs_changed"]);
}

#[test]
fn tool_diagnostics_do_not_carry_outside_content() {
    if !available("cargo", "--version") {
        return;
    }
    // cargo quotes the manifest it failed to parse; one reached outside ROOT must not end up
    // in the graph.
    let w = work("rust");
    let outside = w.0.join("outside.toml");
    fs::write(&outside, "SECRET_TOKEN_VALUE\n").unwrap();
    let manifest = w.repo().join("Cargo.toml");
    let text = fs::read_to_string(&manifest)
        .unwrap()
        .replace("\"legacy\"]", "\"legacy\", \"localdep\"]");
    fs::write(&manifest, text).unwrap();
    fs::write(
        w.repo().join(".gitignore"),
        "/target/\n/Cargo.lock\n/localdep/\n",
    )
    .unwrap();
    git(&w.repo(), &["commit", "-qam", "member"]);
    fs::create_dir_all(w.repo().join("localdep")).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, w.repo().join("localdep/Cargo.toml")).unwrap();
    let (ok, v) = taeguk(&[
        "graph",
        "build",
        w.repo().to_str().unwrap(),
        "--out",
        w.graph().to_str().unwrap(),
        "--lang",
        "rust",
    ]);
    assert!(ok, "{v}");
    let graph = fs::read_to_string(w.graph()).unwrap();
    assert!(!graph.contains("SECRET_TOKEN_VALUE"), "{graph}");
    assert!(!v.to_string().contains("SECRET_TOKEN_VALUE"));
}

#[test]
fn glob_reexport_cycles_finish() {
    if !available("cargo", "--version") {
        return;
    }
    // 17 modules glob re-exporting each other: without state dedupe this takes 16^8 steps.
    let w = work("rust");
    let dir = w.repo().join("core/src/web");
    fs::create_dir_all(&dir).unwrap();
    let mods: String = (0..17).map(|i| format!("pub mod m{i};\n")).collect();
    fs::write(w.repo().join("core/src/web.rs"), mods).unwrap();
    for i in 0..17 {
        let mut text: String = (0..17)
            .filter(|j| *j != i)
            .map(|j| format!("pub use super::m{j}::*;\n"))
            .collect();
        if i == 0 {
            text += "pub fn f() {\n    crate::web::m0::missing::g();\n}\n";
        }
        fs::write(dir.join(format!("m{i}.rs")), text).unwrap();
    }
    // Two names bound to each other through 16 cfg branches each would expand 16^8 times.
    let mut web: String = (0..16)
        .map(|i| format!("#[cfg(any())]\nuse b::p{i} as a;\n#[cfg(any())]\nuse a::q{i} as b;\n"))
        .collect();
    web += "pub fn g() {\n    a::x::y();\n}\n";
    fs::write(w.repo().join("core/src/alias_web.rs"), web).unwrap();
    let lib = w.repo().join("core/src/lib.rs");
    let text = fs::read_to_string(&lib).unwrap() + "pub mod web;\npub mod alias_web;\n";
    fs::write(&lib, text).unwrap();
    git(&w.repo(), &["add", "-A"]);
    git(&w.repo(), &["commit", "-qm", "web"]);
    let started = std::time::Instant::now();
    let g = build(&w, "rust");
    assert_eq!(g["status"], "PASS", "{g}");
    assert!(started.elapsed() < Duration::from_secs(60));
    assert!(
        g["gaps"]
            .as_array()
            .unwrap()
            .iter()
            .any(|x| x["kind"] == "resolution_truncated" && x["file"] == "core/src/alias_web.rs"),
        "{g}"
    );
}

#[test]
fn rebuild_is_stable_and_dirty_trees_are_recorded() {
    if !available("python3", "--version") {
        return;
    }
    let w = work("python");
    // Ignored local environments may link outside ROOT; only special files are refused there.
    fs::write(w.repo().join(".gitignore"), "/.venv/\n").unwrap();
    git(&w.repo(), &["commit", "-qam", "ignore venv"]);
    fs::create_dir_all(w.repo().join(".venv/bin")).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink("/bin/sh", w.repo().join(".venv/bin/python")).unwrap();
    let first = build(&w, "python");
    assert_eq!(first["status"], "PASS", "{first}");
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
fn review_files(v: &Value) -> Vec<String> {
    let files: BTreeSet<String> = v["llm_review"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| g["file"].as_str().unwrap().to_string())
        .collect();
    files.into_iter().collect()
}
fn other_reasons(v: &Value) -> Vec<String> {
    v["reasons"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r.as_str().unwrap().to_string())
        .filter(|r| r != "gaps_need_llm_review")
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
    // Gaps outside the impacted files may hide dependents, so they go to LLM review.
    assert_eq!(v["status"], "UNKNOWN", "{v}");
    assert_eq!(v["reasons"], serde_json::json!(["gaps_need_llm_review"]));
    assert_eq!(
        review_files(&v),
        [
            "plug/run.py",
            "plugrun/run.py",
            "src/app/nspkg/__init__.py",
            "src/app/wilduser.py",
            "tests/test_aliases.py",
            "tests/test_annotated.py",
            "tests/test_bexec.py",
            "tests/test_boolop.py",
            "tests/test_builtins.py",
            "tests/test_cond.py",
            "tests/test_exec.py",
            "tests/test_file.py",
            "tests/test_fromexec.py",
            "tests/test_helper.py",
            "tests/test_importbind.py",
            "tests/test_match.py",
            "tests/test_nested.py",
            "tests/test_order.py",
            "tests/test_param.py",
            "tests/test_patch.py",
            "tests/test_paths.py",
            "tests/test_rebind.py",
            "tests/test_reload.py",
            "tests/test_shadow.py",
            "tests/test_unpack.py",
            "tests/test_walrus.py",
            "tests/test_wild.py"
        ]
    );
    assert_eq!(
        distances(&v),
        set(&[
            ("python:src/app/util.py", 0),
            ("python:src/app/core.py", 1),
            ("python:src/app/cli.py", 2),
            ("python:src/app/sub/deep.py", 2),
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

    // Uncommitted edits can change imports the graph of HEAD does not show.
    fs::write(w.repo().join("common.py"), "import scripts\n").unwrap();
    let edited = impact(&w, &["--changed", "src/app/util.py"]);
    assert_eq!(edited["status"], "UNKNOWN");
    assert_eq!(
        other_reasons(&edited),
        ["working_tree_differs_from_head", "analyzer_inputs_changed"]
    );
    git(&w.repo(), &["checkout", "--", "."]);
    // An untracked source is read by analyzers too, so it also makes the tree differ.
    fs::write(w.repo().join("scripts/extra.py"), "import helper\n").unwrap();
    let untracked = impact(&w, &["--changed", "src/app/util.py"]);
    assert_eq!(
        other_reasons(&untracked),
        ["working_tree_differs_from_head", "analyzer_inputs_changed"]
    );
    fs::remove_file(w.repo().join("scripts/extra.py")).unwrap();

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
            ("python:tests/test_helper.py", 1),
            ("python:tests/test_order.py", 1)
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
    let expected = set(&[
        ("go:example.com/fx/a", 0),
        ("go:example.com/fx/cmd", 1),
        ("go:example.com/fx/c", 1),
        ("go:example.com/fx/b", 2),
    ]);
    assert_eq!(v["status"], "UNKNOWN", "{v}");
    assert_eq!(review_files(&v), ["d/d_tool.go", "f"]);
    assert_eq!(distances(&v), expected);
    // Without gaps the static result is complete for the graph's scope.
    git(&w.repo(), &["rm", "-q", "-r", "d/d_tool.go", "f"]);
    git(&w.repo(), &["commit", "-qm", "drop"]);
    build(&w, "go");
    let v = impact(&w, &["--changed", "a/a.go"]);
    assert_eq!(v["status"], "PASS", "{v}");
    assert_eq!(v["llm_review"], serde_json::json!([]));
    assert_eq!(distances(&v), expected);
}

/// Rewrites one analyzer version and recomputes the digest the way the CLI defines it.
fn with_version(graph: &Path, version: &str) {
    let mut g: Value = serde_json::from_slice(&fs::read(graph).unwrap()).unwrap();
    g["analyzers"][0]["version"] = version.into();
    let body = serde_json::json!({"schema": g["schema"], "commit": g["commit"], "dirty": g["dirty"],
        "dirty_paths": g["dirty_paths"], "read": g["read"], "inputs": g["inputs"], "status": g["status"], "analyzers": g["analyzers"],
        "nodes": g["nodes"], "edges": g["edges"], "gaps": g["gaps"]});
    use sha2::Digest;
    g["digest"] = format!(
        "sha256:{:x}",
        sha2::Sha256::digest(body.to_string().as_bytes())
    )
    .into();
    fs::write(graph, serde_json::to_string(&g).unwrap()).unwrap();
}

#[test]
fn impact_rejects_other_roots_and_tool_versions() {
    if !available("python3", "--version") {
        return;
    }
    let w = work("python");
    build(&w, "python");
    let (graph, sub) = (w.graph(), w.repo().join("scripts"));
    let (ok, v) = taeguk(&[
        "graph",
        "impact",
        graph.to_str().unwrap(),
        sub.to_str().unwrap(),
        "--changed",
        "helper.py",
    ]);
    assert!(ok, "{v}");
    assert_eq!(v["status"], "UNKNOWN");
    assert!(
        v["reasons"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r == "graph_root_differs")
    );

    with_version(&w.graph(), "Python 0.0.0");
    let v = impact(&w, &["--changed", "scripts/helper.py"]);
    assert_eq!(v["status"], "UNKNOWN");
    assert_eq!(other_reasons(&v), ["analyzer_version_changed_or_unknown"]);
}

#[test]
fn graph_input_is_checked_before_use() {
    if !available("python3", "--version") {
        return;
    }
    let w = work("python");
    build(&w, "python");
    // A FIFO or device given as the graph is refused before it is opened.
    let fifo = w.0.join("fifo.json");
    assert!(
        Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success()
    );
    let (ok, v) = taeguk(&[
        "graph",
        "impact",
        fifo.to_str().unwrap(),
        w.repo().to_str().unwrap(),
        "--changed",
        "common.py",
    ]);
    assert!(!ok);
    assert!(v["error"].as_str().unwrap().contains("regular file"), "{v}");
    // An edge to an unknown node is an error, not a crash, even with a recomputed digest.
    let mut g: Value = serde_json::from_slice(&fs::read(w.graph()).unwrap()).unwrap();
    g["edges"].as_array_mut().unwrap().push(
        serde_json::json!({"from": "python:ghost.py", "to": "python:common.py", "kind": "import"}),
    );
    fs::write(w.graph(), serde_json::to_string(&g).unwrap()).unwrap();
    with_version(&w.graph(), g["analyzers"][0]["version"].as_str().unwrap());
    let o = Command::new(env!("CARGO_BIN_EXE_taeguk"))
        .args(["graph", "impact"])
        .arg(w.graph())
        .arg(w.repo())
        .args(["--changed", "common.py"])
        .output()
        .unwrap();
    assert_eq!(
        o.status.code(),
        Some(2),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    assert!(String::from_utf8_lossy(&o.stderr).contains("unknown node"));
    // `read` paths are hashed again by impact; one leaving ROOT is refused, not walked.
    for bad in ["../", "/etc/", "a/../../b", ""] {
        build(&w, "python");
        let mut g: Value = serde_json::from_slice(&fs::read(w.graph()).unwrap()).unwrap();
        g["read"].as_array_mut().unwrap().push(Value::from(bad));
        fs::write(w.graph(), serde_json::to_string(&g).unwrap()).unwrap();
        with_version(&w.graph(), g["analyzers"][0]["version"].as_str().unwrap());
        let (ok, v) = taeguk(&[
            "graph",
            "impact",
            w.graph().to_str().unwrap(),
            w.repo().to_str().unwrap(),
            "--changed",
            "common.py",
        ]);
        assert!(!ok, "{bad}: {v}");
        assert!(v["error"].as_str().unwrap().contains("read path"), "{v}");
    }
}

#[test]
fn large_and_linked_inputs_stay_bounded() {
    if !available("python3", "--version") {
        return;
    }
    // A tracked file grown far beyond any source counts as changed without being read.
    let w = work("python");
    let big = w.repo().join("common.py");
    fs::OpenOptions::new()
        .write(true)
        .open(&big)
        .unwrap()
        .set_len(2 << 30)
        .unwrap();
    let started = std::time::Instant::now();
    let g = build(&w, "python");
    assert!(started.elapsed() < Duration::from_secs(60));
    assert!(
        g["dirty_paths"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p == "common.py"),
        "{g}"
    );
    // `..` right after a link inside ROOT walks from the link target, as the OS does.
    let w = work("python");
    fs::create_dir_all(w.repo().join("sub/deep")).unwrap();
    fs::write(w.repo().join("sub/sibling.py"), "V = 1\n").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink("sub/deep", w.repo().join("link")).unwrap();
    fs::write(
        w.repo().join("dotdot.py"),
        "import sys\nfrom pathlib import Path\n\nsys.path.insert(0, str(Path(__file__).parent / \"link\" / \"..\"))\nimport sibling\n",
    )
    .unwrap();
    git(&w.repo(), &["add", "-A"]);
    git(&w.repo(), &["commit", "-qm", "link"]);
    let g = build(&w, "python");
    assert!(
        g["edges"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["from"] == "python:dotdot.py" && e["to"] == "python:sub/sibling.py"),
        "{g}"
    );
    assert!(
        !g["gaps"]
            .as_array()
            .unwrap()
            .iter()
            .any(|x| x["file"] == "dotdot.py"),
        "{g}"
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
