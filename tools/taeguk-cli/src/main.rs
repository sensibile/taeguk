use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    env, fs,
    io::Read,
    path::{Component, Path},
    process,
};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Counts {
    good: u64,
    bad: u64,
    unknown: u64,
    target: Option<f64>,
}
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum Verdict {
    Pass,
    Fail,
    Unknown,
    NotRun,
    NotApplicable,
}
fn measurement(c: &Counts) -> Result<serde_json::Value, String> {
    if c.target
        .is_some_and(|t| !t.is_finite() || !(0.0..=1.0).contains(&t))
    {
        return Err("target must be finite and between 0 and 1".into());
    }
    let n = c
        .good
        .checked_add(c.bad)
        .and_then(|v| v.checked_add(c.unknown))
        .ok_or("count overflow")?;
    if n > 9_007_199_254_740_991 {
        return Err("total exceeds exact integer range for ratio calculation".into());
    }
    if n == 0 {
        return Ok(
            serde_json::json!({"verdict":"UNKNOWN","reason":"no_eligible_events","total":0}),
        );
    }
    let lower = c.good as f64 / n as f64;
    let upper = (c.good + c.unknown) as f64 / n as f64;
    let (verdict, reason) = match c.target {
        None => (Verdict::Unknown, "target_not_defined"),
        Some(t) if upper < t => (Verdict::Fail, "upper_bound_below_target"),
        Some(t) if lower >= t => (Verdict::Pass, "lower_bound_meets_target"),
        Some(_) => (Verdict::Unknown, "unknown_events_affect_decision"),
    };
    Ok(
        serde_json::json!({"verdict":verdict,"reason":reason,"total":n,
        "good":c.good,"bad":c.bad,"unknown":c.unknown,"lower":lower,"upper":upper,
        "quality":if c.unknown==0 {"VALID"} else {"DEGRADED"},"target":c.target,
        "limits":["caller must validate the population and collection completeness","floating point ratio"]}),
    )
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Packet {
    artifact: String,
    sha256: String,
    checks: Vec<Verdict>,
    effect_confirmed: bool,
}
fn verify(p: &Packet, root: &Path) -> Result<serde_json::Value, String> {
    let rel = Path::new(&p.artifact);
    if p.artifact.is_empty() || rel.components().any(|c| !matches!(c, Component::Normal(_))) {
        return Err("artifact must be a nonempty relative path without traversal".into());
    }
    if p.sha256.len() != 64 || !p.sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("sha256 must have 64 hexadecimal characters".into());
    }
    let base = root.canonicalize().map_err(|e| e.to_string())?;
    let candidate = base.join(rel);
    // Reject every symlink component, including links inside the root.
    let mut component_path = base.clone();
    for component in rel.components() {
        component_path.push(component.as_os_str());
        if fs::symlink_metadata(&component_path)
            .map_err(|e| e.to_string())?
            .file_type()
            .is_symlink()
        {
            return Err("symlink artifact paths are not supported".into());
        }
    }
    let target = candidate.canonicalize().map_err(|e| e.to_string())?;
    if !target.starts_with(&base) || !target.is_file() {
        return Err("artifact is outside root or not a file".into());
    }
    let mut file = fs::File::open(&target).map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = file.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hash.update(&buf[..n]);
    }
    let digest = format!("{:x}", hash.finalize());
    let same = digest.eq_ignore_ascii_case(&p.sha256);
    let (verdict, reason) = if !same {
        (Verdict::Fail, "artifact_mismatch")
    } else if p.checks.contains(&Verdict::Fail) {
        (Verdict::Fail, "failed_check")
    } else if p.checks.is_empty() || p.checks.iter().any(|s| !matches!(s, Verdict::Pass)) {
        (Verdict::Unknown, "missing_or_unconfirmed_checks")
    } else if !p.effect_confirmed {
        (Verdict::Unknown, "effect_not_confirmed")
    } else {
        (Verdict::Pass, "declared_checks_and_artifact_match")
    };
    Ok(
        serde_json::json!({"verdict":verdict,"reason":reason,"actual_sha256":digest,
        "artifact_matches":same,"limits":["declared check statuses are not independently verified","not approval or deployment proof","requires stable artifact during verification"]}),
    )
}
fn read_json<T: for<'a> Deserialize<'a>>(path: &str) -> Result<T, String> {
    let mut data = Vec::new();
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(1_048_577)
        .read_to_end(&mut data)
        .map_err(|e| e.to_string())?;
    if data.len() > 1_048_576 {
        return Err("input exceeds 1 MiB".into());
    }
    serde_json::from_slice(&data).map_err(|e| e.to_string())
}
fn run() -> Result<serde_json::Value, String> {
    let a: Vec<String> = env::args().skip(1).collect();
    match a.as_slice() {
        [cmd, path] if cmd == "measure" => measurement(&read_json::<Counts>(path)?),
        [cmd, path, root] if cmd == "verify" => {
            verify(&read_json::<Packet>(path)?, Path::new(root))
        }
        _ => Err("usage: taeguk measure COUNTS.json | taeguk verify PACKET.json ROOT".into()),
    }
}
fn main() {
    match run() {
        Ok(v) => println!("{v}"),
        Err(e) => {
            eprintln!("{}", serde_json::json!({"error":e}));
            process::exit(2);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn no_data_is_unknown() {
        assert_eq!(
            measurement(&Counts {
                good: 0,
                bad: 0,
                unknown: 0,
                target: Some(0.9)
            })
            .unwrap()["verdict"],
            "UNKNOWN"
        );
    }
    #[test]
    fn unknown_bounds_can_fail() {
        let v = measurement(&Counts {
            good: 8,
            bad: 1,
            unknown: 1,
            target: Some(0.99),
        })
        .unwrap();
        assert_eq!(v["verdict"], "FAIL");
        assert_eq!(v["lower"], 0.8);
        assert_eq!(v["upper"], 0.9);
    }
    #[test]
    fn unknown_blocks_ambiguous_pass() {
        assert_eq!(
            measurement(&Counts {
                good: 8,
                bad: 1,
                unknown: 1,
                target: Some(0.85)
            })
            .unwrap()["verdict"],
            "UNKNOWN"
        );
    }
    #[test]
    fn confirmed_lower_bound_passes() {
        assert_eq!(
            measurement(&Counts {
                good: 9,
                bad: 0,
                unknown: 1,
                target: Some(0.9)
            })
            .unwrap()["verdict"],
            "PASS"
        );
    }
    #[test]
    fn invalid_and_overflow_rejected() {
        assert!(
            measurement(&Counts {
                good: u64::MAX,
                bad: 1,
                unknown: 0,
                target: None
            })
            .is_err()
        );
        assert!(
            measurement(&Counts {
                good: 1,
                bad: 0,
                unknown: 0,
                target: Some(f64::NAN)
            })
            .is_err()
        );
    }
    #[test]
    fn excessive_count_precision_is_rejected() {
        assert!(
            measurement(&Counts {
                good: 9_007_199_254_740_991,
                bad: 1,
                unknown: 0,
                target: Some(1.0)
            })
            .is_err()
        );
    }
    #[test]
    fn missing_target_is_unknown() {
        assert_eq!(
            measurement(&Counts {
                good: 1,
                bad: 0,
                unknown: 0,
                target: None
            })
            .unwrap()["verdict"],
            "UNKNOWN"
        );
    }
    #[test]
    fn verifier_requires_real_file_and_effect() {
        let dir = env::temp_dir().join(format!("taeguk-rust-{}", process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("artifact"), b"abc").unwrap();
        let mut p = Packet {
            artifact: "artifact".into(),
            sha256: format!("{:x}", Sha256::digest(b"abc")),
            checks: vec![Verdict::Pass],
            effect_confirmed: false,
        };
        assert_eq!(verify(&p, &dir).unwrap()["verdict"], "UNKNOWN");
        p.effect_confirmed = true;
        assert_eq!(verify(&p, &dir).unwrap()["verdict"], "PASS");
        p.sha256 = "0".repeat(64);
        assert_eq!(verify(&p, &dir).unwrap()["verdict"], "FAIL");
        p.artifact = "../artifact".into();
        assert!(verify(&p, &dir).is_err());
        fs::remove_dir_all(dir).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn symlink_is_rejected() {
        let dir = env::temp_dir().join(format!("taeguk-link-{}", process::id()));
        fs::create_dir_all(&dir).unwrap();
        std::os::unix::fs::symlink("/etc/hosts", dir.join("artifact")).unwrap();
        let p = Packet {
            artifact: "artifact".into(),
            sha256: "0".repeat(64),
            checks: vec![Verdict::Pass],
            effect_confirmed: true,
        };
        assert!(verify(&p, &dir).is_err());
        fs::remove_dir_all(dir).unwrap();
    }
}
