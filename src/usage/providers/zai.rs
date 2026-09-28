use super::*;

fn credentials() -> Option<(String, Option<String>)> {
    std::env::var("Z_AI_API_KEY")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| auth_text("z-ai", "access"))
        .or_else(|| auth_text("zai", "access"))
        .map(|token| (token, None))
}

pub(super) fn windows(data: &Value) -> Vec<UsageWindow> {
    let mut result = Vec::new();
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
    result
}

pub(super) fn fetch(agent: &ureq::Agent) -> Result<UsageSnapshot, FetchError> {
    let (token, _) = credentials().ok_or_else(failure)?;
    let bearer = format!("Bearer {token}");
    let result = {
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
        windows(&data)
    };
    Ok(snapshot("zai", "z.ai Plan", result, None))
}
