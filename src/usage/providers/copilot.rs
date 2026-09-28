use super::*;

fn credentials() -> Option<(String, Option<String>)> {
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

pub(super) fn windows(data: &Value) -> Vec<UsageWindow> {
    let mut result = Vec::new();
    if let Some(quota) = data["quota_snapshots"].get("premium_interactions") {
        let mut usage = window(
            "Month",
            100. - number(&quota["percent_remaining"]).unwrap_or(0.),
        );
        usage.reset_at = reset_iso(&data["quota_reset_date_utc"]);
        result.push(usage);
    }
    result
}

pub(super) fn fetch(agent: &ureq::Agent) -> Result<UsageSnapshot, FetchError> {
    let (token, _) = credentials().ok_or_else(failure)?;
    let result = windows(&request(
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
    )?);
    Ok(snapshot("copilot", "Copilot Plan", result, None))
}
