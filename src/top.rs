use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::Path,
    process::{Command, Stdio},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tempfile::NamedTempFile;

const FRESH_MS: u64 = 10_000;
const STALE_MS: u64 = 60_000;
const EXPIRE_MS: u64 = 300_000;
const LEASE_MS: u64 = 5_000;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct HostStatus {
    pub cpu: Option<u8>,
    pub memory: Option<u8>,
    pub battery: Option<Battery>,
    pub age_seconds: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Battery {
    pub percent: u8,
    pub charging: bool,
    pub full: bool,
}

#[derive(Serialize, Deserialize)]
struct Record {
    version: u8,
    sampled_at: u64,
    status: HostStatus,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn read(dir: &Path) -> Option<Record> {
    let path = dir.join("status.json");
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
    let record: Record = serde_json::from_slice(&bytes).ok()?;
    let age = now().checked_sub(record.sampled_at)?;
    (bytes.len() <= 1024
        && record.version == 1
        && age <= EXPIRE_MS
        && record.status.cpu.is_none_or(|value| value <= 100)
        && record.status.memory.is_none_or(|value| value <= 100)
        && record
            .status
            .battery
            .as_ref()
            .is_none_or(|battery| battery.percent <= 100))
    .then_some(record)
}

fn ensure_dir(dir: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn lock(dir: &Path) -> Option<fs::File> {
    ensure_dir(dir).ok()?;
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(dir.join("status.lock"))
        .ok()?;
    file.try_lock().ok()?;
    Some(file)
}

pub(crate) fn resolve(dir: &Path, spawn: bool) -> HostStatus {
    let record = read(dir);
    let age = record
        .as_ref()
        .map(|record| now().saturating_sub(record.sampled_at));
    if spawn && age.is_none_or(|age| age >= FRESH_MS) {
        if let Some(guard) = lock(dir) {
            let due =
                read(dir).is_none_or(|record| now().saturating_sub(record.sampled_at) >= FRESH_MS);
            if due {
                let _ = launch(dir);
            }
            drop(guard);
        }
    }
    record.map_or_else(HostStatus::default, |record| {
        let mut status = record.status;
        let age = age.unwrap_or_default();
        status.age_seconds = (age >= STALE_MS).then_some(age / 1000);
        status
    })
}

fn launch(dir: &Path) -> std::io::Result<()> {
    let lease = dir.join("worker.lease");
    if lease.exists() {
        let active = fs::metadata(&lease)
            .and_then(|meta| meta.modified())
            .ok()
            .and_then(|modified| SystemTime::now().duration_since(modified).ok())
            .is_some_and(|elapsed| elapsed < Duration::from_millis(LEASE_MS));
        if active {
            return Ok(());
        }
        fs::remove_file(&lease)?;
    }
    let token = format!("{}-{}", std::process::id(), now());
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lease)?;
    file.write_all(token.as_bytes())?;
    let result = Command::new(std::env::current_exe()?)
        .arg("top-refresh")
        .arg(dir)
        .arg(&token)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    if result.is_err() {
        let _ = fs::remove_file(lease);
    }
    result.map(|_| ())
}

pub fn refresh(dir: &Path, token: &str) -> Result<(), String> {
    if token.is_empty()
        || token.len() > 80
        || !token
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte == b'-')
    {
        return Err("invalid top worker lease".into());
    }
    let status = sample();
    let _guard = lock(dir).ok_or("top cache unavailable")?;
    let lease = dir.join("worker.lease");
    if fs::read_to_string(&lease).ok().as_deref() != Some(token) {
        return Err("top worker lease expired".into());
    }
    let record = Record {
        version: 1,
        sampled_at: now(),
        status,
    };
    let bytes = serde_json::to_vec(&record).map_err(|error| error.to_string())?;
    let result = (|| -> std::io::Result<()> {
        let mut file = NamedTempFile::new_in(dir)?;
        file.write_all(&bytes)?;
        file.persist(dir.join("status.json"))?;
        Ok(())
    })()
    .map_err(|error| error.to_string());
    let _ = fs::remove_file(&lease);
    result
}

fn sample() -> HostStatus {
    let mut system = sysinfo::System::new();
    system.refresh_cpu_usage();
    std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
    system.refresh_cpu_usage();
    system.refresh_memory();
    let percentage = |value: f64| {
        value
            .is_finite()
            .then(|| value.round().clamp(0.0, 100.0) as u8)
    };
    HostStatus {
        cpu: percentage(f64::from(system.global_cpu_usage())),
        memory: (system.total_memory() > 0).then(|| {
            (100.0 * system.used_memory() as f64 / system.total_memory() as f64)
                .round()
                .clamp(0.0, 100.0) as u8
        }),
        battery: battery(),
        age_seconds: None,
    }
}

#[cfg(target_os = "macos")]
fn battery() -> Option<Battery> {
    let mut pmset = Command::new("pmset");
    pmset.args(["-g", "batt"]);
    let limits = crate::process::Limits {
        deadline: std::time::Instant::now() + Duration::from_millis(500),
        stdout: 4096,
        stderr: 0,
    };
    let output = crate::process::run(pmset, limits).ok()?;
    if !output.status.success() {
        return None;
    }
    let output = String::from_utf8(output.stdout).ok()?;
    let line = output.lines().find(|line| line.contains('%'))?;
    let prefix = line.split('%').next()?;
    let percent = prefix
        .rsplit(|c: char| !c.is_ascii_digit())
        .next()?
        .parse()
        .ok()?;
    Some(Battery {
        percent,
        charging: line.contains("charging") && !line.contains("discharging"),
        full: line.contains("charged") || line.contains("finishing charge"),
    })
}

#[cfg(target_os = "linux")]
fn battery() -> Option<Battery> {
    let supplies = fs::read_dir("/sys/class/power_supply").ok()?;
    let mut batteries = Vec::new();
    for entry in supplies.flatten().take(32) {
        let path = entry.path();
        if fs::read_to_string(path.join("type"))
            .ok()
            .is_none_or(|value| value.trim() != "Battery")
        {
            continue;
        }
        if fs::read_to_string(path.join("present"))
            .ok()
            .is_some_and(|value| value.trim() == "0")
        {
            continue;
        }
        let Some(percent) = fs::read_to_string(path.join("capacity"))
            .ok()
            .and_then(|text| text.trim().parse::<u8>().ok())
        else {
            continue;
        };
        let state = fs::read_to_string(path.join("status")).unwrap_or_default();
        if percent <= 100 {
            batteries.push((percent, state));
        }
    }
    if batteries.is_empty() {
        return None;
    }
    let percent = (batteries
        .iter()
        .map(|(value, _)| u32::from(*value))
        .sum::<u32>()
        / batteries.len() as u32) as u8;
    Some(Battery {
        percent,
        charging: batteries
            .iter()
            .any(|(_, state)| state.trim() == "Charging"),
        full: batteries.iter().all(|(_, state)| state.trim() == "Full"),
    })
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn battery() -> Option<Battery> {
    None
}
