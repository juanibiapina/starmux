use super::*;

fn credentials() -> Option<(String, Option<String>)> {
    auth_text("google-gemini-cli", "access")
        .or_else(|| {
            json_file(home().join(".gemini/oauth_creds.json"))
                .and_then(|v| text(&v, "access_token").map(str::to_owned))
        })
        .map(|token| (token, None))
}

pub(super) fn windows(data: &Value) -> Vec<UsageWindow> {
    let mut result = Vec::new();
    for (needle, label) in [("pro", "Pro"), ("flash", "Flash")] {
        let min = data["buckets"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|b| {
                text(b, "modelId").is_some_and(|model| model.to_ascii_lowercase().contains(needle))
            })
            .map(|b| number(&b["remainingFraction"]).unwrap_or(1.))
            .reduce(f64::min);
        if let Some(min) = min {
            result.push(window(label, (1. - min) * 100.));
        }
    }
    result
}

pub(super) fn fetch(agent: &ureq::Agent) -> Result<UsageSnapshot, FetchError> {
    let (token, _) = credentials().ok_or_else(failure)?;
    let bearer = format!("Bearer {token}");
    let result = windows(&request(
        agent,
        "https://cloudcode-pa.googleapis.com/v1internal:retrieveUserQuota",
        &bearer,
        &[],
        Some(&serde_json::json!({})),
    )?);
    Ok(snapshot("gemini", "Gemini Plan", result, None))
}
