use super::*;

fn credentials() -> Option<(String, Option<String>)> {
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

pub(super) fn windows(data: &Value) -> Vec<UsageWindow> {
    let mut result = Vec::new();
    let mut models = std::collections::BTreeMap::<String, (f64, Option<u64>)>::new();
    if let Some(items) = data["models"].as_object() {
        for (id, model) in items {
            if model["isInternal"] == true || id.eq_ignore_ascii_case("tab_flash_lite_preview") {
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
    result
}

pub(super) fn fetch(agent: &ureq::Agent) -> Result<UsageSnapshot, FetchError> {
    let (token, extra) = credentials().ok_or_else(failure)?;
    let bearer = format!("Bearer {token}");
    let result = {
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
        windows(&first.or_else(|_| {
            request(
                agent,
                "https://cloudcode-pa.googleapis.com/v1internal:fetchAvailableModels",
                &bearer,
                &headers,
                Some(&body),
            )
        })?)
    };
    Ok(snapshot("antigravity", "Antigravity", result, None))
}
