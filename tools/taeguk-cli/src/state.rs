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
        _ => Err("usage: taeguk state list | get WORK_ID | history WORK_ID AFTER_REVISION | create FILE.json | command WORK_ID FILE.json".into())
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
    fn malformed_response_and_invalid_token_are_errors() {
        let (base, h) = server("200 OK", "not JSON", "");
        assert!(request(&base, TOKEN, "/api/work-items", None).is_err());
        h.join().unwrap();
        assert!(request("http://127.0.0.1:1", "bad\nsecret", "/api/work-items", None).is_err());
    }
}
