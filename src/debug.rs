use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    hash::{Hash, Hasher},
    io::{Read, Write},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};
use tempfile::NamedTempFile;

pub(crate) const STAGES: [&str; 9] = [
    "tmux", "pi", "pr", "usage", "gob", "commands", "git", "format", "top",
];
const MAX_AGE_NS: u128 = 300_000_000_000;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Diagnostics {
    pub stages: [u64; 9],
}

impl Diagnostics {
    pub fn total(&self) -> u64 {
        self.stages.iter().sum()
    }
}

#[derive(Serialize, Deserialize)]
struct Record {
    version: u8,
    key: String,
    completed_ns: u128,
    diagnostics: Diagnostics,
}

fn now() -> Option<u128> {
    Some(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()?
            .as_nanos(),
    )
}

fn key(socket: &str, client: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    socket.hash(&mut hasher);
    client.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

pub(crate) fn read(dir: &Path, socket: &str, client: &str) -> Option<Diagnostics> {
    let key = key(socket, client);
    read_record(dir, &key).map(|record| record.diagnostics)
}

fn read_record(dir: &Path, key: &str) -> Option<Record> {
    let path = dir.join(format!("{key}.json"));
    let meta = fs::symlink_metadata(&path).ok()?;
    if !meta.file_type().is_file() || meta.len() > 1024 {
        return None;
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .ok()?
        .take(1025)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > 1024 {
        return None;
    }
    let record: Record = serde_json::from_slice(&bytes).ok()?;
    let age = now()?.checked_sub(record.completed_ns)?;
    (record.version == 1
        && record.key == key
        && age <= MAX_AGE_NS
        && record
            .diagnostics
            .stages
            .iter()
            .all(|value| *value <= 10_000_000)
        && record.diagnostics.total() <= 30_000_000)
        .then_some(record)
}

pub(crate) fn write(
    dir: &Path,
    socket: &str,
    client: &str,
    diagnostics: &Diagnostics,
) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
    }
    let key = key(socket, client);
    let completed_ns = now().unwrap_or_default();
    let mut options = OpenOptions::new();
    options.write(true).create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let lock = options.open(dir.join(format!("{key}.lock")))?;
    lock.try_lock()?;
    if read_record(dir, &key).is_some_and(|record| record.completed_ns > completed_ns) {
        return Ok(());
    }
    let target = dir.join(format!("{key}.json"));
    let record = Record {
        version: 1,
        key,
        completed_ns,
        diagnostics: diagnostics.clone(),
    };
    let mut file = NamedTempFile::new_in(dir)?;
    file.write_all(&serde_json::to_vec(&record).map_err(std::io::Error::other)?)?;
    file.persist(target)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{key, read, write, Diagnostics};

    #[test]
    fn expired_corrupt_and_cross_client_records_are_ignored() {
        let tempdir = tempfile::tempdir().unwrap();
        let dir = tempdir.path().to_path_buf();
        std::fs::create_dir_all(&dir).unwrap();
        let expected = Diagnostics {
            stages: [1200, 0, 0, 0, 0, 0, 0, 80, 0],
        };
        write(&dir, "socket", "client", &expected).unwrap();
        assert_eq!(read(&dir, "socket", "client").unwrap().total(), 1280);
        assert!(read(&dir, "socket", "other").is_none());
        let path = dir.join(format!("{}.json", key("socket", "client")));
        let mut record: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        record["completed_ns"] = 0.into();
        std::fs::write(&path, serde_json::to_vec(&record).unwrap()).unwrap();
        assert!(read(&dir, "socket", "client").is_none());
        std::fs::write(&path, b"not json").unwrap();
        assert!(read(&dir, "socket", "client").is_none());
    }
}
