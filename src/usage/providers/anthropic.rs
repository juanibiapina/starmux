use super::*;
#[cfg(target_os = "macos")]
use std::process::Command;

fn credentials() -> Option<(String, Option<String>)> {
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

pub(super) fn windows(data: &Value) -> Vec<UsageWindow> {
    let mut result = Vec::new();
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
    result
}

pub(super) fn fetch(agent: &ureq::Agent) -> Result<UsageSnapshot, FetchError> {
    let (token, _) = credentials().ok_or_else(failure)?;
    let bearer = format!("Bearer {token}");
    let result = windows(&request(
        agent,
        "https://api.anthropic.com/api/oauth/usage",
        &bearer,
        &[("anthropic-beta", "oauth-2025-04-20")],
        None,
    )?);
    Ok(snapshot("anthropic", "Claude Plan", result, None))
}
