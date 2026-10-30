use serde_json::{Value, json};
use std::{env, io::Read, time::Duration};

fn local_base(base: &str) -> Result<String, String> {
    let uri: ureq::http::Uri = base.parse().map_err(|_| "invalid state API address")?;
    let authority = uri.authority().ok_or("state API requires an authority")?;
    if uri.scheme_str() != Some("http")
        || uri.host() != Some("127.0.0.1")
        || authority.as_str().contains('@')
        || !matches!(uri.path(), "" | "/")
        || uri.query().is_some()
        || authority.port_u16() == Some(0)
    {
        return Err("state API must be a plain HTTP loopback base (127.0.0.1) without credentials, path or query".into());
    }
    Ok(format!("http://{authority}"))
}

fn work_id(id: &str) -> Result<(), String> {
    if id.len() != 36
        || !id.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_hexdigit() && !b.is_ascii_uppercase()
            }
        })
    {
        return Err("work id must be a lowercase UUID".into());
    }
    Ok(())
}

fn request(base: &str, token: &str, path: &str, body: Option<&Value>) -> Result<Value, String> {
    let base = local_base(base)?;
    if token.len() < 24 || token.len() > 512 || !token.bytes().all(|b| b.is_ascii_graphic()) {
        return Err("TAEGUK_API_TOKEN must contain 24..512 visible ASCII bytes".into());
    }
    let agent = ureq::Agent::config_builder()
        .proxy(None)
        .max_redirects(0)
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(15)))
        .build()
        .new_agent();
    let url = format!("{base}{path}");
    let response = match body {
        Some(value) => agent
            .post(&url)
            .header("x-taeguk-token", token)
            .send_json(value),
        None => agent.get(&url).header("x-taeguk-token", token).call(),
    };
    let mut response = response.map_err(|_| "state transport failed; outcome may be unknown, reuse the same command id after checking state".to_string())?;
    let status = response.status().as_u16();
    let mut bytes = Vec::new();
    response
        .body_mut()
        .as_reader()
        .take(16_777_217)
        .read_to_end(&mut bytes)
        .map_err(|_| "state response read failed; outcome may be unknown")?;
    if bytes.len() > 16_777_216 {
        return Err("state response exceeds 16 MiB".into());
    }
    if !(200..300).contains(&status) {
        let reason = serde_json::from_slice::<Value>(&bytes)
            .ok()
            .and_then(|v| v.get("error").cloned())
            .unwrap_or(json!("non_success_response"));
        return Err(format!("state HTTP {status}: {reason}"));
    }
    serde_json::from_slice(&bytes).map_err(|_| "state API returned invalid JSON".into())
}

fn evidence_item(source: Value, item: &Value, root: &std::path::Path) -> Value {
    let location = item["location"].as_str().unwrap_or_default();
    let packet = crate::Packet {
        artifact: location.into(),
        sha256: item["sha256"].as_str().unwrap_or_default().into(),
        checks: vec![],
        effect_confirmed: false,
    };
    let (status, reason) = match crate::verify(&packet, root) {
        Ok(v) if v["artifact_matches"] == true => ("PASS", "sha256_matches".to_string()),
        Ok(_) => ("FAIL", "sha256_mismatch".to_string()),
        Err(e) => ("UNKNOWN", e),
    };
    json!({"source":source,"location":location,"status":status,"reason":reason})
}

// Compares registered evidence of the current state with local files.
fn evidence_report(state: &Value, root: &std::path::Path) -> Value {
    let data = &state["data"];
    let mut items = Vec::new();
    for o in data["observations"].as_array().into_iter().flatten() {
        for e in o["document"]["evidence"].as_array().into_iter().flatten() {
            items.push(evidence_item(json!({"observation_id":o["id"]}), e, root));
        }
    }
    if let Some(progress) = data["work"]["document"]["improvement_progress"].as_object() {
        for (phase, entry) in progress {
            for e in entry["evidence"].as_array().into_iter().flatten() {
                items.push(evidence_item(json!({"improvement_phase":phase}), e, root));
            }
        }
    }
    let has = |s: &str| items.iter().any(|i| i["status"] == s);
    let (verdict, reason) = if items.is_empty() {
        ("UNKNOWN", "no_registered_evidence")
    } else if has("FAIL") {
        ("FAIL", "evidence_mismatch")
    } else if has("UNKNOWN") {
        ("UNKNOWN", "evidence_not_checkable")
    } else {
        ("PASS", "registered_evidence_matches")
    };
    json!({"verdict":verdict,"reason":reason,"work_id":data["work"]["id"],
        "revision":data["work"]["revision"],"items":items,
        "limits":["local file identity only, not evidence truth, independence or approval",
            "current observations and improvement entries only; use history for older evidence",
            "locations must be relative paths under ROOT"]})
}

pub fn run(args: &[String]) -> Result<Value, String> {
    let base = env::var("TAEGUK_API_URL").unwrap_or_else(|_| "http://127.0.0.1:4000".into());
    let token = env::var("TAEGUK_API_TOKEN").map_err(|_| "TAEGUK_API_TOKEN is required")?;
    match args {
        [cmd] if cmd == "list" => request(&base, &token, "/api/work-items", None),
        [cmd, id] if cmd == "get" => {
            work_id(id)?; request(&base, &token, &format!("/api/work-items/{id}"), None)
        },
        [cmd, id, cursor] if cmd == "history" => {
            work_id(id)?;
            cursor.parse::<u64>().map_err(|_| "history cursor must be a nonnegative revision")?;
            request(&base, &token, &format!("/api/work-items/{id}/history?after_revision={cursor}"), None)
        },
        [cmd, file] if cmd == "create" => request(&base, &token, "/api/work-items", Some(&crate::read_json::<Value>(file)?)),
        [cmd, id, file] if cmd == "command" => {
            work_id(id)?;
            request(&base, &token, &format!("/api/work-items/{id}/commands"), Some(&crate::read_json::<Value>(file)?))
        },
        [cmd, id, root] if cmd == "verify-evidence" => {
            work_id(id)?;
            let state = request(&base, &token, &format!("/api/work-items/{id}"), None)?;
            Ok(evidence_report(&state, std::path::Path::new(root)))
        },
        _ => Err("usage: taeguk state list | get WORK_ID | history WORK_ID AFTER_REVISION | create FILE.json | command WORK_ID FILE.json | verify-evidence WORK_ID ROOT".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Write, net::TcpListener, thread};
    const TOKEN: &str = "local-test-token-with-24-bytes";
    fn server(status: &str, body: &str, extra: &str) -> (String, thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = format!("http://{}", listener.local_addr().unwrap());
        let reply = format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n{extra}\r\n{body}",
            body.len()
        );
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut bytes = Vec::new();
            loop {
                let mut buf = [0u8; 1024];
                let n = stream.read(&mut buf).unwrap();
                bytes.extend_from_slice(&buf[..n]);
                if bytes.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            stream.write_all(reply.as_bytes()).unwrap();
            String::from_utf8(bytes).unwrap()
        });
        (address, handle)
    }
    #[test]
    fn local_address_rejects_remote_credentials_and_routes() {
        for bad in [
            "http://example.com",
            "https://127.0.0.1",
            "http://name@127.0.0.1",
            "http://127.0.0.1/x",
            "http://127.0.0.1/?x=1",
            "http://127.0.0.1:0",
        ] {
            assert!(local_base(bad).is_err(), "{bad}");
        }
        assert_eq!(
            local_base("http://127.0.0.1:4120/").unwrap(),
            "http://127.0.0.1:4120"
        );
        assert!(work_id("../other").is_err());
    }
    #[test]
    fn reads_real_http_and_sends_token() {
        let (base, h) = server("200 OK", r#"{"data":{"revision":4}}"#, "");
        assert_eq!(
            request(&base, TOKEN, "/api/work-items", None).unwrap()["data"]["revision"],
            4
        );
        assert!(h.join().unwrap().to_lowercase().contains("x-taeguk-token:"));
    }
    #[test]
    fn conflict_is_an_error_without_retry() {
        let (base, h) = server("409 Conflict", r#"{"error":"stale_revision"}"#, "");
        assert!(
            request(&base, TOKEN, "/api/work-items", None)
                .unwrap_err()
                .contains("409")
        );
        h.join().unwrap();
    }
    #[test]
    fn redirect_is_not_followed() {
        let (base, h) = server("302 Found", "{}", "Location: http://127.0.0.1:1/\r\n");
        assert!(
            request(&base, TOKEN, "/api/work-items", None)
                .unwrap_err()
                .contains("302")
        );
        h.join().unwrap();
    }
    #[test]
    fn evidence_report_compares_registered_files() {
        use sha2::{Digest, Sha256};
        let dir = env::temp_dir().join(format!("taeguk-evidence-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("result.log"), b"ok").unwrap();
        let good = format!("{:x}", Sha256::digest(b"ok"));
        let state = |evidence: Value| {
            json!({"data":{"work":{"id":"w","revision":3,"document":{
                "improvement_progress":{"phase":"baseline","baseline":{"evidence":[
                    {"location":"result.log","sha256":good}]}}}},
                "observations":[{"id":"o1","document":{"evidence":evidence}}]}})
        };
        let report = evidence_report(
            &state(json!([{"location":"result.log","sha256":good}])),
            &dir,
        );
        assert_eq!(report["verdict"], "PASS");
        assert_eq!(report["items"].as_array().unwrap().len(), 2);
        let report = evidence_report(
            &state(json!([{"location":"result.log","sha256":"0".repeat(64)}])),
            &dir,
        );
        assert_eq!(report["verdict"], "FAIL");
        let report = evidence_report(
            &state(json!([{"location":"missing.log","sha256":good}])),
            &dir,
        );
        assert_eq!(report["verdict"], "UNKNOWN");
        let report = evidence_report(
            &state(json!([{"location":"/etc/hosts","sha256":good}])),
            &dir,
        );
        assert_eq!(report["verdict"], "UNKNOWN");
        let empty = json!({"data":{"work":{"document":{}},"observations":[]}});
        assert_eq!(
            evidence_report(&empty, &dir)["reason"],
            "no_registered_evidence"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn malformed_response_and_invalid_token_are_errors() {
        let (base, h) = server("200 OK", "not JSON", "");
        assert!(request(&base, TOKEN, "/api/work-items", None).is_err());
        h.join().unwrap();
        assert!(request("http://127.0.0.1:1", "bad\nsecret", "/api/work-items", None).is_err());
    }
    #[test]
    fn run_rejects_non_uuid_work_ids_before_any_request() {
        // docs/workflow-state-usage.md: get, history, command and verify-evidence take WORK_UUID.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let dir = env::temp_dir().join(format!(
            "taeguk-state-id-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&dir).unwrap();
        let command = dir.join("command.json");
        std::fs::write(&command, b"{}").unwrap();
        let command = command.to_str().unwrap().to_string();
        let root = dir.to_str().unwrap().to_string();
        // SAFETY: no other test in this crate reads or writes these variables.
        unsafe {
            env::set_var(
                "TAEGUK_API_URL",
                format!("http://{}", listener.local_addr().unwrap()),
            );
            env::set_var("TAEGUK_API_TOKEN", TOKEN);
        }
        for id in [
            "../x",
            "11111111-1111-4111-8111-11111111111/",
            "11111111-1111-4111-8111-111111111111?after_revision=0",
        ] {
            for args in [
                vec!["get", id],
                vec!["history", id, "0"],
                vec!["command", id, &command],
                vec!["verify-evidence", id, &root],
            ] {
                let args: Vec<String> = args.into_iter().map(String::from).collect();
                assert!(run(&args).is_err(), "{args:?}");
                assert!(
                    matches!(listener.accept(), Err(e) if e.kind() == std::io::ErrorKind::WouldBlock),
                    "{args:?} reached the state API"
                );
            }
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
}
