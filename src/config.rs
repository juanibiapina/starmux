use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub format: String,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub provider: BTreeMap<String, Provider>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub module: BTreeMap<String, ModuleConfig>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Provider {
    pub command: Vec<String>,
    pub cwd: String,
    pub decoder: String,
    pub timeout_ms: u64,
    pub cache: bool,
    pub disabled: bool,
    pub shell: Option<String>,
    pub dependencies: Vec<String>,
    pub failure: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct ModuleConfig {
    pub provider: String,
}

impl Default for Provider {
    fn default() -> Self {
        Self {
            command: vec![],
            cwd: "{pane.path}".into(),
            decoder: "plain".into(),
            timeout_ms: 150,
            cache: true,
            disabled: false,
            shell: None,
            dependencies: vec!["pane.path".into()],
            failure: "preserve".into(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            format: "$sessions$divider".into(),
            provider: BTreeMap::new(),
            module: BTreeMap::new(),
        }
    }
}

pub fn path() -> Option<PathBuf> {
    if let Some(value) = std::env::var_os("STARMUX_CONFIG") {
        return Some(PathBuf::from(value));
    }
    let home = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".config")))?;
    Some(home.join("starmux.toml"))
}

pub fn load() -> Result<Config, String> {
    let Some(path) = path() else {
        return Ok(Config::default());
    };
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Config::default()),
        Err(err) => return Err(format!("{}: {err}", path.display())),
    };
    if text.trim().is_empty() {
        return Ok(Config::default());
    }
    let value: toml::Value = toml::from_str(&text).map_err(|err| err.to_string())?;
    let mut merged = toml::Value::try_from(Config::default()).map_err(|err| err.to_string())?;
    merge(&mut merged, value);
    let config: Config = merged
        .try_into()
        .map_err(|err: toml::de::Error| err.to_string())?;
    validate(&config)?;
    Ok(config)
}

fn merge(base: &mut toml::Value, overlay: toml::Value) {
    match (base, overlay) {
        (toml::Value::Table(base), toml::Value::Table(overlay)) => {
            for (key, value) in overlay {
                if let Some(previous) = base.get_mut(&key) {
                    merge(previous, value);
                } else {
                    base.insert(key, value);
                }
            }
        }
        (base, overlay) => *base = overlay,
    }
}

pub fn modules(format: &str) -> Result<Vec<String>, String> {
    let mut result = Vec::new();
    let mut chars = format.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '$' {
            return Err("format must contain only $module references".into());
        }
        let name: String =
            std::iter::from_fn(|| chars.next_if(|c| c.is_ascii_alphanumeric() || *c == '_'))
                .collect();
        if name.is_empty() {
            return Err("empty module reference".into());
        }
        result.push(name);
    }
    Ok(result)
}

pub fn validate(config: &Config) -> Result<(), String> {
    for name in modules(&config.format)? {
        if !matches!(name.as_str(), "sessions" | "divider") && !config.module.contains_key(&name) {
            return Err(format!("unknown module {name}"));
        }
    }
    for (name, provider) in &config.provider {
        if provider.command.is_empty() {
            return Err(format!("provider {name} has no command"));
        }
        if provider.timeout_ms == 0 || provider.timeout_ms > 60000 {
            return Err(format!("provider {name}: timeout_ms must be 1..60000"));
        }
        if !matches!(provider.decoder.as_str(), "plain" | "json-v1") {
            return Err(format!("provider {name}: unknown decoder"));
        }
        if !matches!(provider.failure.as_str(), "preserve" | "hide") {
            return Err(format!("provider {name}: failure must be preserve or hide"));
        }
        for dep in &provider.dependencies {
            if dep != "pane.path" && dep != "session.id" {
                return Err(format!("provider {name}: unknown dependency {dep}"));
            }
        }
    }
    for (name, module) in &config.module {
        if !config.provider.contains_key(&module.provider) {
            return Err(format!(
                "module {name}: unknown provider {}",
                module.provider
            ));
        }
    }
    Ok(())
}
