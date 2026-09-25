use std::process::Command;

use crate::input::Context;

pub const USAGE: &str =
    "usage: render-query --socket=PATH --client=NAME [--current-session=ID --current-window=ID]";

pub struct Focus {
    session: String,
    window: String,
}

impl Focus {
    pub fn parse(args: &[String]) -> Result<Option<Self>, String> {
        if args.is_empty() {
            return Ok(None);
        }
        if args.len() != 2 {
            return Err(USAGE.into());
        }
        let session = args[0].strip_prefix("--current-session=").ok_or(USAGE)?;
        let window = args[1].strip_prefix("--current-window=").ok_or(USAGE)?;
        if !valid_id(session, '$') || !valid_id(window, '@') {
            return Err(USAGE.into());
        }
        Ok(Some(Self {
            session: session.into(),
            window: window.into(),
        }))
    }

    pub fn verify(&self, context: &Context) -> Result<(), String> {
        let selected = context
            .sessions
            .iter()
            .find(|session| session.id == context.current_session)
            .and_then(|session| session.windows.iter().find(|window| window.selected));
        if context.current_session != self.session
            || selected.is_none_or(|window| window.id != self.window)
        {
            return Err("tmux focus changed during query".into());
        }
        Ok(())
    }
}

fn valid_id(value: &str, prefix: char) -> bool {
    value.strip_prefix(prefix).is_some_and(|digits| {
        !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())
    })
}

pub fn snapshot(socket: &str, client: &str) -> Result<Vec<String>, String> {
    if socket.is_empty() || client.is_empty() {
        return Err("missing tmux socket or client name".into());
    }
    let output = Command::new("tmux")
        .args(["-S", socket, "display-message", "-p", "-c", client, "--"])
        .arg(crate::transport())
        .output()
        .map_err(|e| format!("tmux query failed: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "tmux query failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    if output.stdout.len() > 128 * 1024 {
        return Err("tmux snapshot exceeds 128 KiB".into());
    }
    let snapshot =
        String::from_utf8(output.stdout).map_err(|_| "tmux snapshot is not UTF-8".to_string())?;
    words(&snapshot)
}

// Decode tmux's q/s shell quoting as data; never execute the snapshot in a shell.
fn words(source: &str) -> Result<Vec<String>, String> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut chars = source.chars();
    let mut quote = None;
    let mut started = false;
    while let Some(ch) = chars.next() {
        match (quote, ch) {
            (None, '\'') | (None, '"') => {
                quote = Some(ch);
                started = true;
            }
            (Some(q), c) if q == c => quote = None,
            (None, '\\') | (Some('"'), '\\') => {
                current.push(chars.next().ok_or("trailing shell escape")?);
                started = true;
            }
            (None, c) if c.is_whitespace() => {
                if started {
                    result.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            (_, c) => {
                current.push(c);
                started = true;
            }
        }
    }
    if quote.is_some() {
        return Err("unterminated quoted tmux value".into());
    }
    if started {
        result.push(current);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::words;

    #[test]
    fn shell_quoted_snapshot_values_remain_literal() {
        assert_eq!(
            words("--name='space and '\\''quote' --path='' --literal='#[fg=red]#{oops}$x' --state=working\n").unwrap(),
            [
                "--name=space and 'quote",
                "--path=",
                "--literal=#[fg=red]#{oops}$x",
                "--state=working"
            ]
        );
        assert_eq!(
            words("--name='line\nbreak' --count=1\n").unwrap(),
            ["--name=line\nbreak", "--count=1"]
        );
    }

    #[test]
    fn malformed_quoting_is_rejected() {
        assert!(words("--name='unclosed").is_err());
        assert!(words("--name=unclosed\\").is_err());
    }
}
