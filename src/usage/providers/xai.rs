use super::*;

fn credentials() -> Option<(String, Option<String>)> {
    auth_text("xai", "access")
        .or_else(|| {
            std::env::var("XAI_OAUTH_TOKEN")
                .ok()
                .filter(|s| !s.is_empty())
        })
        .or_else(|| {
            std::env::var("GROK_CLI_OAUTH_TOKEN")
                .ok()
                .filter(|s| !s.is_empty())
        })
        .or_else(|| {
            let path = std::env::var_os("GROK_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home().join(".grok"));
            json_file(path.join("auth.json"))?
                .as_object()?
                .values()
                .find_map(|entry| text(entry, "key").map(str::to_owned))
        })
        .map(|token| (token, None))
}

pub(super) fn month_windows(data: &Value) -> Vec<UsageWindow> {
    let mut result = Vec::new();
    let config = &data["config"];
    if let (Some(limit), Some(used)) = (
        number(&config["monthlyLimit"]["val"]),
        number(&config["used"]["val"]),
    ) {
        if limit > 0. && used >= 0. {
            let mut usage = window("Month", used / limit * 100.);
            usage.reset_at = reset_iso(&config["billingPeriodEnd"]);
            result.push(usage);
        }
    }
    result
}

pub(super) fn week_windows(data: &Value) -> Vec<UsageWindow> {
    let mut result = Vec::new();
    let config = &data["config"];
    if config["currentPeriod"]["type"] == "USAGE_PERIOD_TYPE_WEEKLY" {
        let mut usage = window("Week", number(&config["creditUsagePercent"]).unwrap_or(0.));
        usage.duration_seconds = Some(7 * 86_400);
        usage.reset_at = reset_iso(&config["billingPeriodEnd"])
            .or_else(|| reset_iso(&config["currentPeriod"]["end"]));
        result.push(usage);
    }
    result
}

fn fetch_windows(
    agent: &ureq::Agent,
    bearer: &str,
    monthly_url: &str,
    weekly_url: &str,
) -> Result<Vec<UsageWindow>, FetchError> {
    let headers = [
        ("Accept", "application/json"),
        ("x-xai-token-auth", "xai-grok-cli"),
    ];
    let monthly = match request(agent, monthly_url, bearer, &headers, None) {
        Err(error) if matches!(error.status, Some(401 | 403)) => return Err(error.into()),
        result => result,
    };
    let mut result = monthly.as_ref().ok().map(month_windows).unwrap_or_default();
    let weekly = request(agent, weekly_url, bearer, &headers, None);
    match weekly {
        Ok(data) => result.extend(week_windows(&data)),
        Err(error) if result.is_empty() => return Err(error.into()),
        Err(_) => {}
    }
    if result.is_empty() {
        return Err(failure());
    }
    Ok(result)
}

pub(super) fn fetch(agent: &ureq::Agent) -> Result<UsageSnapshot, FetchError> {
    let (token, _) = credentials().ok_or_else(failure)?;
    let result = fetch_windows(
        agent,
        &format!("Bearer {token}"),
        "https://cli-chat-proxy.grok.com/v1/billing",
        "https://cli-chat-proxy.grok.com/v1/billing?format=credits",
    )?;
    Ok(snapshot("xai", "Grok", result, None))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
    };

    #[test]
    fn unauthorized_monthly_request_does_not_query_weekly_usage() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let observer = listener.try_clone().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut buffer = [0; 4096];
            let size = stream.read(&mut buffer).unwrap();
            assert!(String::from_utf8_lossy(&buffer[..size]).starts_with("GET /monthly "));
            stream.write_all(b"HTTP/1.1 401 Unauthorized\r\nRetry-After: 120\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
        });
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(2)))
            .build()
            .into();
        let error = fetch_windows(
            &agent,
            "Bearer test-token",
            &format!("http://{address}/monthly"),
            &format!("http://{address}/weekly"),
        )
        .err()
        .unwrap();
        server.join().unwrap();
        assert_eq!(error.retry_after, Some(Duration::from_secs(120)));
        observer.set_nonblocking(true).unwrap();
        assert!(
            matches!(observer.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock)
        );
    }

    #[test]
    fn weekly_usage_survives_a_monthly_request_failure() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            for (path, response) in [
                ("/monthly", "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_owned()),
                ("/weekly", {
                    let body = r#"{"config":{"currentPeriod":{"type":"USAGE_PERIOD_TYPE_WEEKLY"},"creditUsagePercent":60}}"#;
                    format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len())
                }),
            ] {
                let (mut stream, _) = listener.accept().unwrap();
                stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
                let mut buffer = [0; 4096];
                let size = stream.read(&mut buffer).unwrap();
                let request = String::from_utf8_lossy(&buffer[..size]).to_ascii_lowercase();
                assert!(request.starts_with(&format!("get {path} http/1.1")));
                assert!(request.contains("authorization: bearer test-token"));
                stream.write_all(response.as_bytes()).unwrap();
            }
        });
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(2)))
            .build()
            .into();
        let windows = fetch_windows(
            &agent,
            "Bearer test-token",
            &format!("http://{address}/monthly"),
            &format!("http://{address}/weekly"),
        )
        .unwrap();
        server.join().unwrap();
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].label, "Week");
        assert_eq!(windows[0].used_percent, 60.);
    }
}
