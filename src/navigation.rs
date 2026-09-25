use std::process::Command;

fn id(value: &str, prefix: char) -> Result<u32, String> {
    value
        .strip_prefix(prefix)
        .ok_or("invalid click target")?
        .parse::<u32>()
        .map_err(|_| "invalid click target".into())
}

// A user range holds 15 bytes; two u32 IDs fit in 13 base-36 digits plus "sw".
pub fn token(session: &str, window: &str) -> Result<String, String> {
    let session = id(session, '$')?;
    let window = id(window, '@')?;
    let mut value = (u64::from(session) << 32) | u64::from(window);
    let mut digits = Vec::new();
    loop {
        let digit = (value % 36) as u8;
        digits.push(if digit < 10 {
            b'0' + digit
        } else {
            b'a' + digit - 10
        });
        value /= 36;
        if value == 0 {
            break;
        }
    }
    digits.reverse();
    Ok(format!("sw{}", String::from_utf8(digits).unwrap()))
}

pub fn target(token: &str) -> Result<String, String> {
    let digits = token.strip_prefix("sw").ok_or("invalid click target")?;
    if digits.is_empty()
        || digits.len() > 13
        || !digits
            .bytes()
            .all(|b| b.is_ascii_digit() || b.is_ascii_lowercase())
    {
        return Err("invalid click target".into());
    }
    let packed = u64::from_str_radix(digits, 36).map_err(|_| "invalid click target")?;
    let session = (packed >> 32) as u32;
    let window = packed as u32;
    if self::token(&format!("${session}"), &format!("@{window}"))? != token {
        return Err("invalid click target".into());
    }
    Ok(format!("${session}:@{window}"))
}

pub fn activate(socket: &str, client: &str, click: &str) -> Result<(), String> {
    if socket.is_empty() || client.is_empty() {
        return Err("missing tmux socket or client name".into());
    }
    let target = target(click)?;
    let output = Command::new("tmux")
        .args(["-S", socket, "switch-client", "-c", client, "-t", &target])
        .output()
        .map_err(|e| format!("tmux switch failed: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "tmux switch failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(())
}
