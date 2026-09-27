//! Provider adapters for subscription usage. Credentials stay in the worker process.
use super::{FetchError, UsageSnapshot, UsageWindow, MAX_AVAILABLE_RESETS};
use serde_json::Value;
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

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

fn credentials(provider: &str) -> Option<(String, Option<String>)> {
    match provider {
        "codex" => {
            if let Some(token) = auth_text("openai-codex", "access") {
                let account = pi_auth()
                    .and_then(|v| text(v.get("openai-codex")?, "accountId").map(str::to_owned));
                return Some((token, account));
            }
            let path = std::env::var_os("CODEX_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home().join(".codex"));
            let auth = json_file(path.join("auth.json"))?;
            let token = text(&auth, "OPENAI_API_KEY")
                .or_else(|| text(auth.get("tokens")?, "access_token"))?;
            Some((
                token.to_owned(),
                auth.get("tokens")
                    .and_then(|v| text(v, "account_id"))
                    .map(str::to_owned),
            ))
        }
        "anthropic" => {
            if let Some(token) = auth_text("anthropic", "access") {
                return Some((token, None));
            }
            #[cfg(target_os = "macos")]
            let keychain = Command::new("security")
                .args([
                    "find-generic-password",
                    "-s",
                    "Claude Code-credentials",
                    "-w",
                ])
                .output()
                .ok()
                .filter(|out| out.status.success())
                .and_then(|out| serde_json::from_slice::<Value>(&out.stdout).ok());
            #[cfg(not(target_os = "macos"))]
            let keychain: Option<Value> = None;
            for source in [
                keychain,
                json_file(home().join(".claude/.credentials.json")),
            ]
            .into_iter()
            .flatten()
            {
                let oauth = &source["claudeAiOauth"];
                if oauth["scopes"]
                    .as_array()
                    .is_some_and(|scopes| scopes.iter().any(|scope| scope == "user:profile"))
                {
                    if let Some(token) = text(oauth, "accessToken") {
                        return Some((token.to_owned(), None));
                    }
                }
            }
            None
        }
        "gemini" => auth_text("google-gemini-cli", "access")
            .or_else(|| {
                json_file(home().join(".gemini/oauth_creds.json"))
                    .and_then(|v| text(&v, "access_token").map(str::to_owned))
            })
            .map(|token| (token, None)),
        "copilot" => {
            let auth = pi_auth();
            let token = auth
                .as_ref()
                .and_then(|a| a.get("github-copilot"))
                .and_then(|a| text(a, "refresh").or_else(|| text(a, "access")))
                .map(str::to_owned);
            token
                .or_else(|| {
                    let config = std::env::var_os("XDG_CONFIG_HOME")
                        .map(PathBuf::from)
                        .unwrap_or_else(|| home().join(".config"));
                    [
                        config.join("github-copilot/hosts.json"),
                        home().join(".github-copilot/hosts.json"),
                    ]
                    .into_iter()
                    .find_map(|path| {
                        let hosts = json_file(path)?;
                        let entries = hosts.as_object()?;
                        entries
                            .iter()
                            .find(|(host, _)| host.eq_ignore_ascii_case("github.com"))
                            .or_else(|| {
                                entries
                                    .iter()
                                    .find(|(host, _)| host.eq_ignore_ascii_case("api.github.com"))
                            })
                            .map(|(_, entry)| entry)
                            .and_then(host_token)
                            .or_else(|| entries.values().find_map(host_token))
                    })
                })
                .map(|token| (token, None))
        }
        "antigravity" => {
            let auth = pi_auth()?;
            let entry = auth.get("google-antigravity")?;
            let token = entry.as_str().or_else(|| {
                ["access", "accessToken", "token", "key"]
                    .iter()
                    .find_map(|key| text(entry, key))
            })?;
            Some((
                token.to_owned(),
                text(entry, "projectId")
                    .or_else(|| text(entry, "project"))
                    .map(str::to_owned),
            ))
        }
        "zai" => std::env::var("Z_AI_API_KEY")
            .ok()
            .filter(|s| !s.is_empty())
            .or_else(|| auth_text("z-ai", "access"))
            .or_else(|| auth_text("zai", "access"))
            .map(|token| (token, None)),
        "xai" => auth_text("xai", "access")
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
            .map(|token| (token, None)),
        _ => None,
    }
}

fn host_token(entry: &Value) -> Option<String> {
    ["oauth_token", "user_token", "github_token", "token"]
        .iter()
        .find_map(|key| text(entry, key).map(str::to_owned))
}

fn failure() -> FetchError {
    FetchError { retry_after: None }
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

fn codex_available_resets(data: &Value) -> Option<u32> {
    data["rate_limit_reset_credits"]["available_count"]
        .as_u64()
        .filter(|count| *count <= u64::from(MAX_AVAILABLE_RESETS))
        .map(|count| count as u32)
}

fn windows(provider: &str, data: &Value) -> Vec<UsageWindow> {
    let mut result = Vec::new();
    match provider {
        "codex" => {
            for (key, secondary) in [("primary_window", false), ("secondary_window", true)] {
                let Some(w) = data["rate_limit"].get(key).filter(|w| w.is_object()) else {
                    continue;
                };
                let (Some(seconds), Some(used)) = (
                    number(&w["limit_window_seconds"]).filter(|seconds| *seconds > 0.),
                    number(&w["used_percent"]),
                ) else {
                    continue;
                };
                let hours = (seconds / 3600.).round() as u32;
                let label = if secondary && hours >= 144 {
                    "Week".to_owned()
                } else if secondary && hours >= 24 {
                    "Day".to_owned()
                } else {
                    format!("{hours}h")
                };
                let mut usage = window(label, used);
                usage.duration_seconds = Some(seconds as u64);
                usage.reset_at = reset_seconds(&w["reset_at"]);
                result.push(usage);
            }
        }
        "anthropic" => {
            for (key, label, seconds) in [
                ("five_hour", "5h", 5 * 3600),
                ("seven_day", "Week", 7 * 86_400),
            ] {
                if let Some(percent) = number(&data[key]["utilization"]) {
                    let mut usage = window(label, percent);
                    usage.duration_seconds = Some(seconds);
                    usage.reset_at = reset_iso(&data[key]["resets_at"]);
                    result.push(usage);
                }
            }
            if data["extra_usage"]["is_enabled"] == true {
                let extra = &data["extra_usage"];
                let status = if number(&data["five_hour"]["utilization"]).unwrap_or(0.) >= 99. {
                    "active"
                } else {
                    "on"
                };
                let used = number(&extra["used_credits"]).unwrap_or(0.) / 100.;
                let label = match number(&extra["monthly_limit"]).filter(|n| *n > 0.) {
                    Some(limit) => format!("Extra [{status}] {used:.2}/{:.2}", limit / 100.),
                    None => format!("Extra [{status}] {used:.2}"),
                };
                result.push(window(label, number(&extra["utilization"]).unwrap_or(0.)));
            }
        }
        "gemini" => {
            for (needle, label) in [("pro", "Pro"), ("flash", "Flash")] {
                let min = data["buckets"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|b| {
                        text(b, "modelId")
                            .is_some_and(|model| model.to_ascii_lowercase().contains(needle))
                    })
                    .map(|b| number(&b["remainingFraction"]).unwrap_or(1.))
                    .reduce(f64::min);
                if let Some(min) = min {
                    result.push(window(label, (1. - min) * 100.));
                }
            }
        }
        "copilot" => {
            if let Some(quota) = data["quota_snapshots"].get("premium_interactions") {
                let mut usage = window(
                    "Month",
                    100. - number(&quota["percent_remaining"]).unwrap_or(0.),
                );
                usage.reset_at = reset_iso(&data["quota_reset_date_utc"]);
                result.push(usage);
            }
        }
        "antigravity" => {
            let mut models = std::collections::BTreeMap::<String, (f64, Option<u64>)>::new();
            if let Some(items) = data["models"].as_object() {
                for (id, model) in items {
                    if model["isInternal"] == true
                        || id.eq_ignore_ascii_case("tab_flash_lite_preview")
                    {
                        continue;
                    }
                    let name = text(model, "displayName").unwrap_or(id);
                    if name.eq_ignore_ascii_case("tab_flash_lite_preview") {
                        continue;
                    }
                    let fraction = number(&model["quotaInfo"]["remainingFraction"]).unwrap_or(1.);
                    let reset = reset_iso(&model["quotaInfo"]["resetTime"]);
                    models
                        .entry(name.to_owned())
                        .and_modify(|current| {
                            if fraction < current.0 {
                                *current = (fraction, reset);
                            } else if fraction == current.0 {
                                current.1 = match (current.1, reset) {
                                    (Some(a), Some(b)) => Some(a.min(b)),
                                    (a, b) => a.or(b),
                                };
                            }
                        })
                        .or_insert((fraction, reset));
                }
            }
            result.extend(models.into_iter().map(|(label, (fraction, reset))| {
                let mut usage = window(label, (1. - fraction) * 100.);
                usage.reset_at = reset;
                usage
            }));
        }
        "zai" => {
            if data["success"] == true && data["code"] == 200 {
                for limit in data["data"]["limits"].as_array().into_iter().flatten() {
                    let label = match limit["type"].as_str() {
                        Some("TOKENS_LIMIT") => "Tokens",
                        Some("TIME_LIMIT") => "Monthly",
                        _ => continue,
                    };
                    let mut usage = window(label, number(&limit["percentage"]).unwrap_or(0.));
                    usage.reset_at = reset_iso(&limit["nextResetTime"])
                        .or_else(|| reset_millis(&limit["nextResetTime"]));
                    result.push(usage);
                }
            }
        }
        "xai-month" => {
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
        }
        "xai-week" => {
            let config = &data["config"];
            if config["currentPeriod"]["type"] == "USAGE_PERIOD_TYPE_WEEKLY" {
                let mut usage = window("Week", number(&config["creditUsagePercent"]).unwrap_or(0.));
                usage.duration_seconds = Some(7 * 86_400);
                usage.reset_at = reset_iso(&config["billingPeriodEnd"])
                    .or_else(|| reset_iso(&config["currentPeriod"]["end"]));
                result.push(usage);
            }
        }
        _ => {}
    }
    result
}

fn run_kiro(args: &[&str], timeout: Duration) -> Result<Output, FetchError> {
    let mut child = Command::new("kiro-cli")
        .args(args)
        .env("TERM", "xterm-256color")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|_| failure())?;
    let deadline = Instant::now() + timeout;
    loop {
        if child.try_wait().map_err(|_| failure())?.is_some() {
            return child.wait_with_output().map_err(|_| failure());
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(failure());
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn strip_ansi(input: &str) -> String {
    let mut result = String::new();
    let mut chars = input.chars();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            match chars.next() {
                Some('[') => {
                    for code in chars.by_ref() {
                        if ('@'..='~').contains(&code) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    for code in chars.by_ref() {
                        if code == '\u{7}' {
                            break;
                        }
                    }
                }
                _ => {}
            }
        } else {
            result.push(ch);
        }
    }
    result
}

fn kiro() -> Result<UsageSnapshot, FetchError> {
    let whoami = run_kiro(&["whoami"], Duration::from_secs(5))?;
    if !whoami.status.success() {
        return Err(failure());
    }
    let output = run_kiro(
        &["chat", "--no-interactive", "/usage"],
        Duration::from_secs(10),
    )?;
    if !output.status.success() {
        return Err(failure());
    }
    let plain = strip_ansi(&String::from_utf8_lossy(&output.stdout));
    let percent = plain
        .split('█')
        .next_back()
        .and_then(|s| s.trim_start().split('%').next())
        .and_then(|s| s.parse::<f64>().ok())
        .or_else(|| {
            let start = plain.find("covered in plan")?;
            let preceding = &plain[..start];
            let open = preceding.rfind('(')?;
            let (used, total) = preceding[open + 1..].split_once(" of ")?;
            let total: f64 = total.trim().parse().ok()?;
            if total > 0. {
                Some(used.trim().parse::<f64>().ok()? / total * 100.)
            } else {
                None
            }
        })
        .unwrap_or(0.);
    Ok(UsageSnapshot {
        provider: "kiro".into(),
        display_name: "Kiro Plan".into(),
        windows: vec![window("Credits", percent)],
        available_resets: None,
    })
}

pub(crate) fn fetch(provider: &str, _context: Option<&str>) -> Result<UsageSnapshot, FetchError> {
    if provider == "kiro" {
        return kiro();
    }
    let (token, extra) = credentials(provider).ok_or_else(failure)?;
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(5)))
        .http_status_as_error(false)
        .build()
        .into();
    let agent = &agent;
    let bearer = format!("Bearer {token}");
    let mut available_resets = None;
    let mut result = match provider {
        "codex" => {
            let mut headers = vec![("Accept", "application/json")];
            if let Some(ref account) = extra {
                headers.push(("ChatGPT-Account-Id", account));
            }
            let data = request(
                agent,
                "https://chatgpt.com/backend-api/wham/usage",
                &bearer,
                &headers,
                None,
            )?;
            available_resets = codex_available_resets(&data);
            windows(provider, &data)
        }
        "anthropic" => windows(
            provider,
            &request(
                agent,
                "https://api.anthropic.com/api/oauth/usage",
                &bearer,
                &[("anthropic-beta", "oauth-2025-04-20")],
                None,
            )?,
        ),
        "gemini" => windows(
            provider,
            &request(
                agent,
                "https://cloudcode-pa.googleapis.com/v1internal:retrieveUserQuota",
                &bearer,
                &[],
                Some(&serde_json::json!({})),
            )?,
        ),
        "copilot" => windows(
            provider,
            &request(
                agent,
                "https://api.github.com/copilot_internal/user",
                &format!("token {token}"),
                &[
                    ("Editor-Version", "vscode/1.96.2"),
                    ("User-Agent", "GitHubCopilotChat/0.26.7"),
                    ("X-Github-Api-Version", "2025-04-01"),
                    ("Accept", "application/json"),
                ],
                None,
            )?,
        ),
        "antigravity" => {
            let body = extra.map_or_else(
                || serde_json::json!({}),
                |project| serde_json::json!({"project": project}),
            );
            let headers = [
                ("User-Agent", "antigravity/1.11.5 darwin/arm64"),
                (
                    "X-Goog-Api-Client",
                    "google-cloud-sdk vscode_cloudshelleditor/0.1",
                ),
                (
                    "Client-Metadata",
                    r#"{"ideType":"IDE_UNSPECIFIED","platform":"PLATFORM_UNSPECIFIED","pluginType":"GEMINI"}"#,
                ),
            ];
            let first = request(
                agent,
                "https://daily-cloudcode-pa.sandbox.googleapis.com/v1internal:fetchAvailableModels",
                &bearer,
                &headers,
                Some(&body),
            );
            windows(
                provider,
                &first.or_else(|_| {
                    request(
                        agent,
                        "https://cloudcode-pa.googleapis.com/v1internal:fetchAvailableModels",
                        &bearer,
                        &headers,
                        Some(&body),
                    )
                })?,
            )
        }
        "zai" => {
            let data = request(
                agent,
                "https://api.z.ai/api/monitor/usage/quota/limit",
                &bearer,
                &[("Accept", "application/json")],
                None,
            )?;
            if data["success"] != true || data["code"] != 200 {
                return Err(failure());
            }
            windows(provider, &data)
        }
        "xai" => {
            let headers = [
                ("Accept", "application/json"),
                ("x-xai-token-auth", "xai-grok-cli"),
            ];
            let monthly = request(
                agent,
                "https://cli-chat-proxy.grok.com/v1/billing",
                &bearer,
                &headers,
                None,
            );
            if let Err(error) = &monthly {
                if matches!(error.status, Some(401 | 403)) {
                    return Err(FetchError {
                        retry_after: error.fetch.retry_after,
                    });
                }
            }
            let mut result = monthly
                .as_ref()
                .ok()
                .map(|data| windows("xai-month", data))
                .unwrap_or_default();
            let weekly = request(
                agent,
                "https://cli-chat-proxy.grok.com/v1/billing?format=credits",
                &bearer,
                &headers,
                None,
            );
            match weekly {
                Ok(data) => result.extend(windows("xai-week", &data)),
                Err(error) if result.is_empty() => return Err(error.into()),
                Err(_) => {}
            }
            if result.is_empty() {
                return Err(failure());
            }
            result
        }
        _ => return Err(failure()),
    };
    result.truncate(16);
    let display_name = match provider {
        "codex" => "Codex Plan",
        "anthropic" => "Claude Plan",
        "gemini" => "Gemini Plan",
        "copilot" => "Copilot Plan",
        "antigravity" => "Antigravity",
        "zai" => "z.ai Plan",
        "xai" => "Grok",
        _ => provider,
    };
    Ok(UsageSnapshot {
        provider: provider.into(),
        display_name: display_name.into(),
        windows: result,
        available_resets,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_provider_windows() {
        let codex = windows(
            "codex",
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
        let anthropic = windows(
            "anthropic",
            &json!({"five_hour":{"utilization":12,"resets_at":"2026-09-30T06:00:06Z"},"seven_day":{"utilization":80}}),
        );
        assert_eq!(anthropic.len(), 2);
        assert_eq!(anthropic[0].reset_at, Some(1790748006));
        assert_eq!(anthropic[1].duration_seconds, Some(7 * 86_400));
        assert_eq!(windows("gemini", &json!({"buckets":[{"modelId":"gemini-pro","remainingFraction":0.8},{"modelId":"gemini-pro","remainingFraction":0.5},{"modelId":"gemini-flash","remainingFraction":0.9}]}))[0].used_percent, 50.);
        assert_eq!(
            windows(
                "copilot",
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
                codex_available_resets(
                    &json!({"rate_limit_reset_credits":{"available_count":value}})
                ),
                expected
            );
        }
        assert_eq!(codex_available_resets(&json!({})), None);
    }

    #[test]
    fn codex_omits_null_and_incomplete_secondary_windows() {
        let primary = json!({"limit_window_seconds":604800,"used_percent":78});
        for secondary in [
            Value::Null,
            json!({"limit_window_seconds":86400}),
            json!({"used_percent":0}),
        ] {
            let parsed = windows(
                "codex",
                &json!({"rate_limit":{"primary_window":primary,"secondary_window":secondary}}),
            );
            assert_eq!(parsed.len(), 1);
            assert_eq!(parsed[0].label, "168h");
            assert_eq!(parsed[0].used_percent, 78.);
        }
        let parsed = windows(
            "codex",
            &json!({"rate_limit":{"secondary_window":{"limit_window_seconds":86400,"used_percent":0}}}),
        );
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].label, "Day");
        assert_eq!(parsed[0].used_percent, 0.);
    }

    #[test]
    fn strips_kiro_terminal_colors() {
        assert_eq!(strip_ansi("\u{1b}[32m████ 72%\u{1b}[0m"), "████ 72%");
    }

    #[test]
    fn parses_remaining_providers() {
        assert_eq!(windows("antigravity", &json!({"models":{"hidden":{"isInternal":true},"a":{"displayName":"Pro","quotaInfo":{"remainingFraction":0.4}},"b":{"displayName":"Pro","quotaInfo":{"remainingFraction":0.2}}}}))[0].used_percent, 80.);
        assert_eq!(windows("zai", &json!({"success":true,"code":200,"data":{"limits":[{"type":"TOKENS_LIMIT","percentage":65}]}}))[0].label, "Tokens");
        assert_eq!(
            windows(
                "xai-month",
                &json!({"config":{"monthlyLimit":{"val":100},"used":{"val":20}}})
            )[0]
            .used_percent,
            20.
        );
        assert_eq!(windows("xai-week", &json!({"config":{"currentPeriod":{"type":"USAGE_PERIOD_TYPE_WEEKLY"},"creditUsagePercent":60}}))[0].used_percent, 60.);
    }
}
