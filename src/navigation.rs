// tmux user ranges hold at most 15 bytes. Recheck the current pane and URL on click.
pub(crate) fn pr_token(pane: &str, url: &str, index: usize) -> Result<String, String> {
    if index >= 16
        || !pane
            .strip_prefix('%')
            .is_some_and(|id| !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()))
        || crate::pr_state::parse_url(url).is_none()
    {
        return Err("invalid PR click target".into());
    }
    let mut hash = 0xcbf29ce484222325u64;
    for byte in pane.bytes().chain([0]).chain(url.bytes()) {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
    }
    Ok(format!("sr{index:02x}{:010x}", hash & 0xffffffffff))
}

pub(crate) fn pr_target<'a>(
    token: &str,
    pane: &str,
    urls: &'a [String],
) -> Result<&'a str, String> {
    if token.len() != 14 || !token.starts_with("sr") {
        return Err("invalid PR click target".into());
    }
    let index = usize::from_str_radix(&token[2..4], 16).map_err(|_| "invalid PR click target")?;
    let url = urls
        .get(index)
        .ok_or("PR click target is no longer present")?;
    if pr_token(pane, url, index)? != token {
        return Err("PR click target is no longer present".into());
    }
    Ok(url)
}

pub(crate) fn file_token(
    kind: &str,
    pane: &str,
    path: &std::path::Path,
    index: usize,
) -> Result<String, String> {
    if !matches!(kind, "sl" | "ss")
        || index >= 16
        || !pane
            .strip_prefix('%')
            .is_some_and(|id| !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()))
    {
        return Err("invalid file click target".into());
    }
    let file = path.to_str().ok_or("invalid file click target")?;
    if !path.is_absolute() {
        return Err("invalid file click target".into());
    }
    let mut hash = 0xcbf29ce484222325u64;
    for byte in pane.bytes().chain([0]).chain(file.bytes()) {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
    }
    Ok(format!("{kind}{index:02x}{:010x}", hash & 0xffffffffff))
}

pub(crate) fn file_index(token: &str) -> Result<usize, String> {
    if token.len() != 14 || !(token.starts_with("sl") || token.starts_with("ss")) {
        return Err("invalid file click target".into());
    }
    usize::from_str_radix(&token[2..4], 16).map_err(|_| "invalid file click target".into())
}

pub(crate) fn session_token(session: &str) -> Result<String, String> {
    Ok(format!("st{}", id(session, '$')?))
}

pub(crate) fn session_target(token: &str) -> Result<String, String> {
    let digits = token.strip_prefix("st").ok_or("invalid click target")?;
    let id = id(&format!("${digits}"), '$')?;
    let target = format!("${id}");
    if session_token(&target)? != token {
        return Err("invalid click target".into());
    }
    Ok(target)
}

fn id(value: &str, prefix: char) -> Result<u32, String> {
    value
        .strip_prefix(prefix)
        .ok_or("invalid click target")?
        .parse::<u32>()
        .map_err(|_| "invalid click target".into())
}

// A user range holds 15 bytes; two u32 IDs fit in 13 base-36 digits plus "sw".
pub(crate) fn token(session: &str, window: &str) -> Result<String, String> {
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

pub(crate) fn pane_token(pane: &str, window: &str) -> Result<String, String> {
    let pane = id(pane, '%')?;
    let window = id(window, '@')?;
    let encoded = token(&format!("${pane}"), &format!("@{window}"))?;
    Ok(format!("sp{}", &encoded[2..]))
}

pub(crate) fn pane_target(value: &str) -> Result<(String, String), String> {
    let digits = value.strip_prefix("sp").ok_or("invalid click target")?;
    let encoded = format!("sw{digits}");
    let packed = target(&encoded)?;
    let (pane, window) = packed.split_once(':').ok_or("invalid click target")?;
    Ok((format!("%{}", &pane[1..]), window.to_owned()))
}

pub(crate) fn target(token: &str) -> Result<String, String> {
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
