use super::{Packet, Verdict, verify};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::BTreeSet, path::Path};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FileRef {
    path: String,
    sha256: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Check {
    id: String,
    status: Verdict,
    reason: String,
    oracle: String,
    command: Option<String>,
    evidence: Vec<FileRef>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Requirement {
    id: String,
    expectation: String,
    status: Verdict,
    check_ids: Vec<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Question {
    id: String,
    impact: String,
    investigation: String,
    next_action: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AuditPacket {
    schema_version: u32,
    target_version: String,
    criteria_source: String,
    artifacts: Vec<FileRef>,
    requirements: Vec<Requirement>,
    checks: Vec<Check>,
    open_questions: Vec<Question>,
    effect_status: Verdict,
    effect_reason: String,
}
fn nonempty(text: &str) -> Result<(), String> {
    if text.trim().is_empty() {
        Err("required text is empty".into())
    } else {
        Ok(())
    }
}
fn unique(ids: impl Iterator<Item = impl AsRef<str>>) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for id in ids {
        let id = id.as_ref();
        if id.is_empty()
            || !id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c))
        {
            return Err("IDs must be nonempty ASCII letters, digits, hyphen or underscore".into());
        }
        if !seen.insert(id.to_owned()) {
            return Err("duplicate ID".into());
        }
    }
    Ok(())
}
fn file_matches(file: &FileRef, root: &Path) -> Result<bool, String> {
    let result = verify(
        &Packet {
            artifact: file.path.clone(),
            sha256: file.sha256.clone(),
            checks: vec![],
            effect_confirmed: false,
        },
        root,
    )?;
    Ok(result["artifact_matches"] == true)
}
pub(super) fn audit(packet: &AuditPacket, root: &Path) -> Result<Value, String> {
    if packet.schema_version != 1 {
        return Err("unsupported audit schema version".into());
    }
    nonempty(&packet.target_version)?;
    nonempty(&packet.criteria_source)?;
    nonempty(&packet.effect_reason)?;
    unique(packet.checks.iter().map(|c| &c.id))?;
    unique(packet.requirements.iter().map(|r| &r.id))?;
    unique(packet.open_questions.iter().map(|q| &q.id))?;
    let mut artifact_paths = BTreeSet::new();
    let mut artifacts_match = !packet.artifacts.is_empty();
    for file in &packet.artifacts {
        if !artifact_paths.insert(&file.path) {
            return Err("duplicate artifact path".into());
        }
        artifacts_match &= file_matches(file, root)?;
    }
    let mut checks = Vec::new();
    let mut linked_checks = BTreeSet::new();
    for check in &packet.checks {
        nonempty(&check.reason)?;
        nonempty(&check.oracle)?;
        if let Some(command) = &check.command {
            nonempty(command)?;
        }
        let mut evidence_matches = !check.evidence.is_empty();
        let mut evidence_paths = BTreeSet::new();
        for file in &check.evidence {
            if !evidence_paths.insert(&file.path) {
                return Err("duplicate evidence path in check".into());
            }
            evidence_matches &= file_matches(file, root)?;
        }
        let observed_claim = matches!(check.status, Verdict::Pass | Verdict::Fail);
        let linked = artifacts_match && evidence_matches;
        if linked {
            linked_checks.insert(check.id.as_str());
        }
        checks.push(json!({"id":check.id,"status":check.status,
            "evidence_linked":linked,"reason":check.reason,"oracle":check.oracle,
            "gap":if observed_claim && !linked { Some("observed_claim_without_matching_files") } else { None }}));
    }
    let mut requirements = Vec::new();
    for req in &packet.requirements {
        nonempty(&req.expectation)?;
        unique(req.check_ids.iter())?;
        let mut support = !req.check_ids.is_empty();
        for id in &req.check_ids {
            let check = packet
                .checks
                .iter()
                .find(|c| &c.id == id)
                .ok_or("requirement references an unknown check")?;
            support &= linked_checks.contains(id.as_str()) && check.status == Verdict::Pass;
        }
        requirements.push(json!({"id":req.id,"expectation":req.expectation,
            "status":req.status,"pass_claim_linked":req.status == Verdict::Pass && support,
            "gap":if req.status == Verdict::Pass && !support { Some("pass_claim_without_linked_pass_checks") } else { None }}));
    }
    for question in &packet.open_questions {
        nonempty(&question.impact)?;
        nonempty(&question.investigation)?;
        nonempty(&question.next_action)?;
    }
    Ok(
        json!({"schema_version":1,"target_version":packet.target_version,
        "criteria_source":packet.criteria_source,"artifacts_match":artifacts_match,
        "checks":checks,"requirements":requirements,
        "open_questions":packet.open_questions.iter().map(|q|json!({"id":q.id,
            "impact":q.impact,"investigation":q.investigation,"next_action":q.next_action})).collect::<Vec<_>>(),
        "effect_status":packet.effect_status,"effect_reason":packet.effect_reason,
        "limits":["reference integrity only; no product completion verdict",
            "commands and declared statuses are not executed or authenticated",
            "matching evidence can be irrelevant or fabricated; semantic review remains necessary",
            "target version and criteria source are caller declarations",
            "requires trusted stable local files; no approval or deployment authority"]}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    use std::{
        fs,
        sync::atomic::{AtomicUsize, Ordering},
    };
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    fn fixture(run: impl FnOnce(Value, &Path)) {
        let dir = std::env::temp_dir().join(format!(
            "taeguk-audit-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("artifact"), "version one").unwrap();
        fs::write(dir.join("evidence"), "observed result").unwrap();
        let doc = json!({"schema_version":1,"target_version":"local-v1",
            "criteria_source":"agreed task","artifacts":[{"path":"artifact","sha256":format!("{:x}",Sha256::digest(b"version one"))}],
            "checks":[{"id":"C1","status":"PASS","reason":"observed expected behavior","oracle":"user example",
                "command":"test command","evidence":[{"path":"evidence","sha256":format!("{:x}",Sha256::digest(b"observed result"))}]}],
            "requirements":[{"id":"R1","expectation":"answer user question","status":"PASS","check_ids":["C1"]}],
            "open_questions":[],"effect_status":"UNKNOWN","effect_reason":"not yet used by user"});
        run(doc, &dir);
        fs::remove_dir_all(dir).unwrap();
    }
    fn result(doc: Value, root: &Path) -> Result<Value, String> {
        audit(&serde_json::from_value(doc).unwrap(), root)
    }
    #[test]
    fn linked_checks_do_not_claim_completion_or_effect() {
        fixture(|doc, root| {
            let output = result(doc, root).unwrap();
            assert_eq!(output["requirements"][0]["pass_claim_linked"], true);
            assert_eq!(output["effect_status"], "UNKNOWN");
            assert!(output.get("verdict").is_none());
        });
    }
    #[test]
    fn evidence_and_artifact_drift_break_claim_links() {
        fixture(|doc, root| {
            fs::write(root.join("evidence"), "changed").unwrap();
            let output = result(doc.clone(), root).unwrap();
            assert_eq!(output["checks"][0]["evidence_linked"], false);
            assert_eq!(output["requirements"][0]["pass_claim_linked"], false);
            fs::write(root.join("artifact"), "changed").unwrap();
            assert_eq!(result(doc, root).unwrap()["artifacts_match"], false);
        });
    }
    #[test]
    fn declarations_without_files_or_support_stay_unlinked() {
        fixture(|mut doc, root| {
            doc["checks"][0]["evidence"] = json!([]);
            assert_eq!(
                result(doc.clone(), root).unwrap()["requirements"][0]["pass_claim_linked"],
                false
            );
            doc["requirements"][0]["check_ids"] = json!([]);
            assert_eq!(
                result(doc.clone(), root).unwrap()["requirements"][0]["pass_claim_linked"],
                false
            );
            doc["artifacts"] = json!([]);
            assert_eq!(result(doc, root).unwrap()["artifacts_match"], false);
        });
    }
    #[test]
    fn statuses_are_preserved_without_false_pass_support() {
        fixture(|doc, root| {
            for status in ["FAIL", "UNKNOWN", "NOT_RUN", "NOT_APPLICABLE"] {
                let mut variant = doc.clone();
                variant["checks"][0]["status"] = json!(status);
                let output = result(variant, root).unwrap();
                assert_eq!(output["checks"][0]["status"], status);
                assert_eq!(output["requirements"][0]["pass_claim_linked"], false);
            }
        });
    }
    #[test]
    fn ambiguous_ids_and_uninvestigated_questions_are_rejected() {
        fixture(|doc, root| {
            let mut duplicate = doc.clone();
            duplicate["checks"]
                .as_array_mut()
                .unwrap()
                .push(doc["checks"][0].clone());
            assert!(result(duplicate, root).is_err());
            let mut dangling = doc.clone();
            dangling["requirements"][0]["check_ids"] = json!(["absent"]);
            assert!(result(dangling, root).is_err());
            let mut question = doc;
            question["open_questions"] = json!([{"id":"Q1","impact":"changes meaning","investigation":" ","next_action":"ask user"}]);
            assert!(result(question, root).is_err());
        });
    }
    #[test]
    fn unsafe_or_missing_evidence_is_an_input_error() {
        fixture(|doc, root| {
            for file in ["../outside", "missing"] {
                let mut variant = doc.clone();
                variant["checks"][0]["evidence"][0]["path"] = json!(file);
                assert!(result(variant, root).is_err());
            }
            #[cfg(unix)]
            {
                std::os::unix::fs::symlink(root.join("evidence"), root.join("link")).unwrap();
                let mut variant = doc;
                variant["checks"][0]["evidence"][0]["path"] = json!("link");
                assert!(result(variant, root).is_err());
            }
        });
    }
    #[test]
    fn ids_outside_the_documented_characters_are_rejected() {
        // docs/evidence-review.md: IDs are ASCII letters, digits, underscore and hyphen.
        fixture(|doc, root| {
            assert!(result(doc.clone(), root).is_ok());
            let question = |id: &str| {
                let mut variant = doc.clone();
                variant["open_questions"] = json!([{"id":id,"impact":"changes meaning",
                    "investigation":"read the plan","next_action":"ask user"}]);
                variant
            };
            assert!(result(question("Q_1-a"), root).is_ok());
            for id in ["C 1", "C1.", "C/1", "\u{c7}1", "C1\u{200b}"] {
                let mut check = doc.clone();
                check["checks"][0]["id"] = json!(id);
                check["requirements"][0]["check_ids"] = json!([id]);
                assert!(result(check, root).is_err(), "check {id:?}");
                let mut requirement = doc.clone();
                requirement["requirements"][0]["id"] = json!(id);
                assert!(result(requirement, root).is_err(), "requirement {id:?}");
                assert!(result(question(id), root).is_err(), "question {id:?}");
            }
        });
    }
    #[test]
    fn unsupported_schema_versions_are_rejected() {
        // docs/evidence-review.md: schema_version is currently 1.
        fixture(|doc, root| {
            assert!(result(doc.clone(), root).is_ok());
            for version in [0, 2] {
                let mut variant = doc.clone();
                variant["schema_version"] = json!(version);
                assert!(result(variant, root).is_err(), "{version}");
            }
        });
    }
}
