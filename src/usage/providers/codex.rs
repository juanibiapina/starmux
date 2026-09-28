use super::super::MAX_AVAILABLE_RESETS;
use super::*;

pub(super) fn available_resets(data: &Value) -> Option<u32> {
    data["rate_limit_reset_credits"]["available_count"]
        .as_u64()
        .filter(|count| *count <= u64::from(MAX_AVAILABLE_RESETS))
        .map(|count| count as u32)
}

fn credentials() -> Option<(String, Option<String>)> {
    if let Some(token) = auth_text("openai-codex", "access") {
        let account =
            pi_auth().and_then(|v| text(v.get("openai-codex")?, "accountId").map(str::to_owned));
        return Some((token, account));
    }
    let path = std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".codex"));
    let auth = json_file(path.join("auth.json"))?;
    let token =
        text(&auth, "OPENAI_API_KEY").or_else(|| text(auth.get("tokens")?, "access_token"))?;
    Some((
        token.to_owned(),
        auth.get("tokens")
            .and_then(|v| text(v, "account_id"))
            .map(str::to_owned),
    ))
}

pub(super) fn windows(data: &Value) -> Vec<UsageWindow> {
    let mut result = Vec::new();
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
    result
}

pub(super) fn fetch(agent: &ureq::Agent) -> Result<UsageSnapshot, FetchError> {
    let (token, extra) = credentials().ok_or_else(failure)?;
    let bearer = format!("Bearer {token}");
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
    let available_resets = available_resets(&data);
    let windows = windows(&data);
    Ok(snapshot("codex", "Codex Plan", windows, available_resets))
}
