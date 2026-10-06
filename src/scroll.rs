use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    hash::{Hash, Hasher},
    io::{Read, Write},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};
use tempfile::NamedTempFile;

const MAX_OFFSET: usize = 10_000;
const MAX_AGE_SECS: u64 = 24 * 60 * 60;

#[derive(Serialize, Deserialize)]
struct Record {
    version: u8,
    key: String,
    updated: u64,
    offset: usize,
    #[serde(default)]
    max: Option<usize>,
    #[serde(default)]
    generation: u64,
    #[serde(default)]
    worker_until_ms: u64,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
struct State {
    offset: usize,
    max: Option<usize>,
    generation: u64,
    worker_until_ms: u64,
}

pub(crate) struct Move {
    pub changed: bool,
    pub start_worker: bool,
}

fn key(socket: &str, client: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    socket.hash(&mut hasher);
    client.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn now() -> Option<u64> {
    Some(SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs())
}

fn now_ms() -> Option<u64> {
    Some(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()?
            .as_millis() as u64,
    )
}

fn read_at(dir: &Path, key: &str) -> Option<State> {
    let path = dir.join(format!("{key}.json"));
    let meta = fs::symlink_metadata(&path).ok()?;
    if !meta.file_type().is_file() || meta.len() > 256 {
        return None;
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .ok()?
        .take(257)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > 256 {
        return None;
    }
    let record: Record = serde_json::from_slice(&bytes).ok()?;
    let age = now()?.checked_sub(record.updated)?;
    (record.version == 1
        && record.key == key
        && age <= MAX_AGE_SECS
        && record.offset <= MAX_OFFSET
        && record.max.is_none_or(|max| max <= MAX_OFFSET))
    .then_some(State {
        offset: record.offset,
        max: record.max,
        generation: record.generation,
        worker_until_ms: record.worker_until_ms,
    })
}

pub(crate) fn read(dir: &Path, socket: &str, client: &str) -> usize {
    read_at(dir, &key(socket, client))
        .unwrap_or_default()
        .offset
}

fn write_at(dir: &Path, key: &str, state: State) -> std::io::Result<()> {
    let target = dir.join(format!("{key}.json"));
    let record = Record {
        version: 1,
        key: key.to_owned(),
        updated: now().unwrap_or_default(),
        offset: state.offset,
        max: state.max,
        generation: state.generation,
        worker_until_ms: state.worker_until_ms,
    };
    let mut file = NamedTempFile::new_in(dir)?;
    file.write_all(&serde_json::to_vec(&record).map_err(std::io::Error::other)?)?;
    file.persist(target)?;
    Ok(())
}

fn update(
    socket: &str,
    client: &str,
    change: impl FnOnce(State) -> State,
    dir: &Path,
) -> std::io::Result<(State, State)> {
    fs::create_dir_all(dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
    }
    let key = key(socket, client);
    let mut options = OpenOptions::new();
    options.write(true).create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let lock = options.open(dir.join(format!("{key}.lock")))?;
    lock.lock_exclusive()?;
    let current = read_at(dir, &key).unwrap_or_default();
    let mut next = change(current);
    next.offset = next.offset.min(MAX_OFFSET);
    if next != current {
        write_at(dir, &key, next)?;
    }
    Ok((current, next))
}

#[derive(Clone, Copy)]
pub enum ScrollDirection {
    Up,
    Down,
}

pub(crate) fn move_by(
    socket: &str,
    client: &str,
    direction: ScrollDirection,
    worker: bool,
    dir: &Path,
) -> std::io::Result<Move> {
    let now = now_ms().unwrap_or_default();
    let (previous, next) = update(
        socket,
        client,
        |mut state| {
            let offset = match direction {
                ScrollDirection::Down => state
                    .offset
                    .saturating_add(1)
                    .min(state.max.unwrap_or(MAX_OFFSET)),
                ScrollDirection::Up => state.offset.saturating_sub(1),
            };
            if offset != state.offset {
                state.offset = offset;
                state.generation = state.generation.wrapping_add(1);
                if worker && state.worker_until_ms <= now {
                    state.worker_until_ms = now.saturating_add(2000);
                }
            }
            state
        },
        dir,
    )?;
    Ok(Move {
        changed: next.offset != previous.offset,
        start_worker: next.worker_until_ms != previous.worker_until_ms,
    })
}

pub(crate) fn worker_generation(dir: &Path, socket: &str, client: &str) -> Option<u64> {
    read_at(dir, &key(socket, client)).map(|state| state.generation)
}

pub(crate) fn worker_next(
    dir: &Path,
    socket: &str,
    client: &str,
    painted: u64,
) -> std::io::Result<bool> {
    let now = now_ms().unwrap_or_default();
    let (_, next) = update(
        socket,
        client,
        |mut state| {
            state.worker_until_ms = if state.generation == painted {
                0
            } else {
                now.saturating_add(2000)
            };
            state
        },
        dir,
    )?;
    Ok(next.worker_until_ms != 0)
}

pub(crate) fn stop_worker(dir: &Path, socket: &str, client: &str) {
    let _ = update(
        socket,
        client,
        |mut state| {
            state.worker_until_ms = 0;
            state
        },
        dir,
    );
}

pub(crate) fn set_bound(dir: &Path, socket: &str, client: &str, max: usize) -> std::io::Result<()> {
    let max = max.min(MAX_OFFSET);
    if read_at(dir, &key(socket, client))
        .is_some_and(|state| state.max == Some(max) && state.offset <= max)
    {
        return Ok(());
    }
    let _ = update(
        socket,
        client,
        |mut state| {
            state.max = Some(max);
            state.offset = state.offset.min(max);
            state
        },
        dir,
    )?;
    Ok(())
}
