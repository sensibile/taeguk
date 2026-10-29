use super::{Analyzer, Edge, Gap, Node, Output, Partial, TempDir, tool_version};
use serde::Deserialize;
use std::{
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

const SCRIPT: &str = include_str!("python_graph.py");

#[derive(Deserialize)]
struct Report {
    roots: Vec<String>,
    edges: Vec<(String, String)>,
    gaps: Vec<ReportGap>,
    errors: Vec<String>,
    /// Links inside ROOT followed while resolving paths.
    #[serde(default)]
    links: Vec<String>,
}
#[derive(Deserialize)]
struct ReportGap {
    kind: String,
    file: String,
    detail: String,
}

pub(super) fn analyze(root: &Path, tracked: &[String]) -> (Analyzer, Result<Partial, String>) {
    let mut a = Analyzer::new(
        "python",
        "python3",
        &["python3", "-I", "-c", "<embedded python_graph.py>"],
    );
    a.version = tool_version("python");
    let result = run(root, tracked, &mut a);
    (a, result)
}

fn run(root: &Path, tracked: &[String], a: &mut Analyzer) -> Result<Partial, String> {
    let files: Vec<&String> = tracked.iter().filter(|f| f.ends_with(".py")).collect();
    let input = serde_json::json!({"root": root, "files": files}).to_string();
    let dir = TempDir::new("python")?;
    let mut child = Command::new("python3")
        .args(["-I", "-c", SCRIPT])
        // Outside ROOT, so the user's environment picks python3, not files in the repository.
        .current_dir(&dir.0)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot run python3: {e}"))?;
    child
        .stdin
        .take()
        .ok_or("python3 stdin unavailable")?
        .write_all(input.as_bytes())
        .map_err(|e| e.to_string())?;
    let o = child.wait_with_output().map_err(|e| e.to_string())?;
    let o = Output {
        code: o.status.code(),
        stdout: o.stdout,
        stderr: String::from_utf8_lossy(&o.stderr).into_owned(),
    };
    a.exit_code = o.code;
    if o.code != Some(0) {
        return Err(format!(
            "python import analysis failed: {}",
            o.stderr.trim()
        ));
    }
    let r: Report = serde_json::from_slice(&o.stdout)
        .map_err(|e| format!("cannot parse python analysis: {e}"))?;
    if !r.errors.is_empty() {
        return Err(format!("python parse errors: {}", r.errors.join("; ")));
    }
    a.command.push(format!(
        "roots={}",
        r.roots
            .iter()
            .map(|r| if r.is_empty() { "." } else { r })
            .collect::<Vec<_>>()
            .join(",")
    ));
    Ok(Partial {
        // pyproject.toml decides the package roots, also when git ignores it; recorded even when
        // absent, so adding one later changes the fingerprint.
        // Links followed while resolving `sys.path` entries are recorded by their target text.
        read: std::iter::once("pyproject.toml".to_string())
            .chain(r.links)
            .collect(),
        nodes: files
            .iter()
            .map(|f| Node::new("python", f.as_str(), vec![f.to_string()]))
            .collect(),
        edges: r
            .edges
            .iter()
            .map(|(f, t)| Edge::new(&format!("python:{f}"), &format!("python:{t}"), "import"))
            .collect(),
        gaps: r
            .gaps
            .into_iter()
            .map(|g| Gap::new("python", &g.kind, Some(&g.file), g.detail))
            .collect(),
    })
}
