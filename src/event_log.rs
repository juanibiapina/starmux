use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_LINE_BYTES: usize = 4096;

#[derive(Default)]
pub(crate) struct Events {
    pub(crate) pane: String,
    lines: Vec<String>,
}

impl Events {
    pub(crate) fn missing(&mut self, source: &str, reason: &str) {
        if self.lines.len() < 16 {
            self.lines
                .push(format!("source={} reason={}", value(source), value(reason)));
        }
    }

    pub(crate) fn result<T>(
        &mut self,
        source: &str,
        result: Result<T, String>,
    ) -> Result<T, String> {
        if let Err(reason) = &result {
            self.missing(source, reason);
        }
        result
    }

    pub(crate) fn availability(
        &mut self,
        usage: &[crate::usage::UsageRow],
        top: Option<&crate::top::HostStatus>,
    ) {
        for row in usage {
            if row.unavailable {
                self.missing("usage", &format!("provider={} unavailable", row.provider));
            } else if let Some(failure) = row.refresh_failure {
                self.missing("usage", &format!("provider={} {failure:?}", row.provider));
            }
        }
        if top.is_some_and(|status| status.cpu.is_none() || status.memory.is_none()) {
            self.missing("top", "CPU or memory reading unavailable");
        }
    }

    pub(crate) fn timings(&mut self, diagnostics: &crate::Diagnostics, total_us: u128) {
        if total_us < 150_000 && diagnostics.stages.iter().all(|us| *us < 50_000) {
            return;
        }
        let mut line = format!("slow total_us={total_us}");
        for (stage, us) in crate::debug::STAGES.iter().zip(diagnostics.stages) {
            line.push_str(&format!(" {stage}_us={us}"));
        }
        self.lines.push(line);
    }

    pub(crate) fn write(&self, path: Option<&Path>, identity: &[(&str, &str)]) {
        if self.lines.is_empty() {
            return;
        }
        let Some(path) = path else { return };
        let _ = self.append(path, identity);
    }

    fn append(&self, path: &Path, identity: &[(&str, &str)]) -> std::io::Result<()> {
        fs::create_dir_all(
            path.parent()
                .ok_or_else(|| std::io::Error::other("no log parent"))?,
        )?;
        let mut options = OpenOptions::new();
        options.create(true).append(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(path)?;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let mut prefix = format!(
            "unix_ms={timestamp} pid={} pane={}",
            std::process::id(),
            value(&self.pane)
        );
        for (key, text) in identity {
            prefix.push_str(&format!(" {key}={}", value(text)));
        }
        for line in &self.lines {
            let mut record = format!("{prefix} {line}");
            while record.len() >= MAX_LINE_BYTES {
                record.pop();
            }
            record.push('\n');
            file.write_all(record.as_bytes())?;
        }
        Ok(())
    }
}

fn value(text: &str) -> String {
    format!("{:?}", text.chars().take(256).collect::<String>())
}
