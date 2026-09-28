//! Provider adapters for subscription usage. Credentials stay in the worker process.
use super::{FetchError, UsageSnapshot, UsageWindow};
use serde_json::Value;
use std::{fs, path::PathBuf, time::Duration};

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default()
}

fn json_file(path: PathBuf) -> Option<Value> {
    serde_json::from_slice(&fs::read(path).ok()?).ok()
}

fn pi_auth() -> Option<Value> {
    json_file(home().join(".pi/agent/auth.json"))
}

fn text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key)?.as_str().filter(|s| !s.is_empty())
}

fn auth_text(provider: &str, key: &str) -> Option<String> {
    text(pi_auth()?.get(provider)?, key).map(str::to_owned)
}

fn host_token(entry: &Value) -> Option<String> {
    ["oauth_token", "user_token", "github_token", "token"]
        .iter()
        .find_map(|key| text(entry, key).map(str::to_owned))
}

fn failure() -> FetchError {
    FetchError { retry_after: None }
}
fn snapshot(
    provider: &str,
    display_name: &str,
    mut windows: Vec<UsageWindow>,
    available_resets: Option<u32>,
) -> UsageSnapshot {
    windows.truncate(super::MAX_WINDOWS);
    UsageSnapshot {
        provider: provider.into(),
        display_name: display_name.into(),
        windows,
        available_resets,
    }
}

fn window(label: impl Into<String>, percent: f64) -> UsageWindow {
    UsageWindow {
        label: label.into(),
        used_percent: if percent.is_finite() {
            percent.clamp(0.0, 100.0)
        } else {
            0.0
        },
        duration_seconds: None,
        reset_at: None,
    }
}

fn reset_iso(value: &Value) -> Option<u64> {
    use time::{format_description::well_known::Rfc3339, OffsetDateTime};
    let timestamp = OffsetDateTime::parse(value.as_str()?, &Rfc3339)
        .ok()?
        .unix_timestamp();
    u64::try_from(timestamp).ok()
}

fn reset_seconds(value: &Value) -> Option<u64> {
    value.as_u64().filter(|seconds| *seconds > 0)
}

fn reset_millis(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .map(|millis| millis / 1000)
        .filter(|seconds| *seconds > 0)
}

fn number(value: &Value) -> Option<f64> {
    value.as_f64().filter(|n| n.is_finite())
}

struct RequestError {
    status: Option<u16>,
    fetch: FetchError,
}

impl From<RequestError> for FetchError {
    fn from(error: RequestError) -> Self {
        error.fetch
    }
}

fn request(
    agent: &ureq::Agent,
    url: &str,
    token: &str,
    headers: &[(&str, &str)],
    body: Option<&Value>,
) -> Result<Value, RequestError> {
    let mut response = if let Some(body) = body {
        let mut req = agent.post(url).header("Authorization", token);
        for (key, value) in headers {
            req = req.header(*key, *value);
        }
        req.send_json(body)
    } else {
        let mut req = agent.get(url).header("Authorization", token);
        for (key, value) in headers {
            req = req.header(*key, *value);
        }
        req.call()
    }
    .map_err(|_| RequestError {
        status: None,
        fetch: failure(),
    })?;
    if !response.status().is_success() {
        let status = response.status().as_u16();
        let retry_after = response
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|seconds| *seconds > 0)
            .map(Duration::from_secs);
        return Err(RequestError {
            status: Some(status),
            fetch: FetchError { retry_after },
        });
    }
    response
        .body_mut()
        .with_config()
        .limit(1024 * 1024)
        .read_json::<Value>()
        .map_err(|_| RequestError {
            status: None,
            fetch: failure(),
        })
}

mod anthropic;
mod antigravity;
mod codex;
mod copilot;
mod gemini;
mod kiro;
mod xai;
mod zai;

pub(crate) fn fetch(provider: &str, _context: Option<&str>) -> Result<UsageSnapshot, FetchError> {
    if provider == "kiro" {
        return kiro::fetch();
    }
    if !super::PROVIDERS.contains(&provider) {
        return Err(failure());
    }
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(5)))
        .http_status_as_error(false)
        .build()
        .into();
    match provider {
        "codex" => codex::fetch(&agent),
        "anthropic" => anthropic::fetch(&agent),
        "gemini" => gemini::fetch(&agent),
        "copilot" => copilot::fetch(&agent),
        "antigravity" => antigravity::fetch(&agent),
        "zai" => zai::fetch(&agent),
        "xai" => xai::fetch(&agent),
        _ => Err(failure()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_provider_windows() {
        let codex = codex::windows(
            &json!({"rate_limit":{"primary_window":{"used_percent":25,"limit_window_seconds":18000,"reset_at":1790748006},"secondary_window":{"used_percent":70,"limit_window_seconds":604800}}}),
        );
        assert_eq!(
            codex
                .iter()
                .map(|w| (&*w.label, w.used_percent))
                .collect::<Vec<_>>(),
            vec![("5h", 25.), ("Week", 70.)]
        );
        assert_eq!(codex[0].duration_seconds, Some(18_000));
        assert_eq!(codex[0].reset_at, Some(1790748006));
        let anthropic = anthropic::windows(
            &json!({"five_hour":{"utilization":12,"resets_at":"2026-09-30T06:00:06Z"},"seven_day":{"utilization":80}}),
        );
        assert_eq!(anthropic.len(), 2);
        assert_eq!(anthropic[0].reset_at, Some(1790748006));
        assert_eq!(anthropic[1].duration_seconds, Some(7 * 86_400));
        assert_eq!(gemini::windows(&json!({"buckets":[{"modelId":"gemini-pro","remainingFraction":0.8},{"modelId":"gemini-pro","remainingFraction":0.5},{"modelId":"gemini-flash","remainingFraction":0.9}]}))[0].used_percent, 50.);
        assert_eq!(
            copilot::windows(
                &json!({"quota_snapshots":{"premium_interactions":{"percent_remaining":75}}})
            )[0]
            .used_percent,
            25.
        );
    }

    #[test]
    fn sends_provider_request_and_observes_retry_after() {
        use std::io::{Read, Write};
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut buffer = [0; 4096];
            let size = stream.read(&mut buffer).unwrap();
            let request = String::from_utf8_lossy(&buffer[..size]).to_ascii_lowercase();
            assert!(request.starts_with("post /usage http/1.1"));
            assert!(request.contains("authorization: bearer test-token"));
            assert!(request.contains("content-type: application/json"));
            stream.write_all(b"HTTP/1.1 429 Too Many Requests\r\nRetry-After: 120\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
        });
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(2)))
            .build()
            .into();
        let error = request(
            &agent,
            &format!("http://{address}/usage"),
            "Bearer test-token",
            &[],
            Some(&json!({})),
        )
        .err()
        .unwrap();
        server.join().unwrap();
        assert_eq!(error.status, Some(429));
        assert_eq!(error.fetch.retry_after, Some(Duration::from_secs(120)));
    }

    #[test]
    fn codex_reports_available_resets() {
        for (value, expected) in [
            (json!(1), Some(1)),
            (json!(0), Some(0)),
            (json!(9999), Some(9999)),
            (json!(null), None),
            (json!(-1), None),
            (json!("1"), None),
            (json!(10000), None),
        ] {
            assert_eq!(
                codex::available_resets(
                    &json!({"rate_limit_reset_credits":{"available_count":value}})
                ),
                expected
            );
        }
        assert_eq!(codex::available_resets(&json!({})), None);
    }

    #[test]
    fn codex_omits_null_and_incomplete_secondary_windows() {
        let primary = json!({"limit_window_seconds":604800,"used_percent":78});
        for secondary in [
            Value::Null,
            json!({"limit_window_seconds":86400}),
            json!({"used_percent":0}),
        ] {
            let parsed = codex::windows(
                &json!({"rate_limit":{"primary_window":primary,"secondary_window":secondary}}),
            );
            assert_eq!(parsed.len(), 1);
            assert_eq!(parsed[0].label, "168h");
            assert_eq!(parsed[0].used_percent, 78.);
        }
        let parsed = codex::windows(
            &json!({"rate_limit":{"secondary_window":{"limit_window_seconds":86400,"used_percent":0}}}),
        );
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].label, "Day");
        assert_eq!(parsed[0].used_percent, 0.);
    }

    #[test]
    fn strips_kiro_terminal_colors() {
        assert_eq!(kiro::strip_ansi("\u{1b}[32m████ 72%\u{1b}[0m"), "████ 72%");
    }

    #[test]
    fn parses_remaining_providers() {
        assert_eq!(antigravity::windows(&json!({"models":{"hidden":{"isInternal":true},"a":{"displayName":"Pro","quotaInfo":{"remainingFraction":0.4}},"b":{"displayName":"Pro","quotaInfo":{"remainingFraction":0.2}}}}))[0].used_percent, 80.);
        assert_eq!(zai::windows(&json!({"success":true,"code":200,"data":{"limits":[{"type":"TOKENS_LIMIT","percentage":65}]}}))[0].label, "Tokens");
        assert_eq!(
            xai::month_windows(&json!({"config":{"monthlyLimit":{"val":100},"used":{"val":20}}}))
                [0]
            .used_percent,
            20.
        );
        assert_eq!(xai::week_windows(&json!({"config":{"currentPeriod":{"type":"USAGE_PERIOD_TYPE_WEEKLY"},"creditUsagePercent":60}}))[0].used_percent, 60.);
    }
}
