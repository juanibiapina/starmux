use std::{
    fs,
    io::{BufRead, BufReader},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

fn snapshot() -> Vec<&'static str> {
    vec![
        "--input-version=1",
        "--width=30",
        "--client-width=80",
        "--client-height=24",
        "--current-session=$0",
        "--current-pane=%0",
        "--pane-path=/tmp",
        "--session-count=1",
        "--session-id=$0",
        "--session-name=hello",
        "--window-count=1",
        "--window-id=@0",
        "--window-index=0",
        "--window-name=world",
        "--selected=1",
        "--window-pane=%0",
        "--window-path=/tmp",
        "--pi-state=",
        "--window-icon=",
        "--end-window",
        "--end-session",
    ]
}

fn temp() -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "starmux-test-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn first_complete_line_arrives_before_slow_provider_and_new_line_replaces_it() {
    let dir = temp();
    let config = dir.join("config.toml");
    fs::write(&config, "format = \"$sessions$divider$custom\"\n[provider.demo]\ncommand = [\"/bin/sh\", \"-c\", \"sleep 0.6; printf 'ready#[fg=red]\\\\nmore'\"]\ncwd = \"/tmp\"\ndecoder = \"plain\"\ntimeout_ms = 1000\ncache = false\n[module.custom]\nprovider = \"demo\"\n").unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_starmux"))
        .arg("render")
        .args(snapshot())
        .env("STARMUX_CONFIG", &config)
        .env("STARMUX_PROGRESSIVE", "1")
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let started = Instant::now();
    let mut lines = BufReader::new(child.stdout.take().unwrap());
    let mut first = String::new();
    lines.read_line(&mut first).unwrap();
    assert!(
        started.elapsed() < Duration::from_millis(500),
        "first snapshot took {:?}: {first}",
        started.elapsed()
    );
    assert!(first.contains("world"));
    assert!(!first.contains("ready"));
    let mut settled = String::new();
    lines.read_line(&mut settled).unwrap();
    assert!(settled.contains("ready##[fg=red]"), "{settled}");
    assert!(settled.contains("#[nl]"));
    assert!(child.wait().unwrap().success());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn failed_provider_does_not_corrupt_navigation() {
    let dir = temp();
    let config = dir.join("config.toml");
    fs::write(&config, "format = \"$sessions$custom\"\n[provider.demo]\ncommand = [\"/bin/sh\", \"-c\", \"echo secret >&2; exit 9\"]\ncwd = \"/tmp\"\ndecoder = \"plain\"\ncache = false\n[module.custom]\nprovider = \"demo\"\n").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_starmux"))
        .arg("render")
        .args(snapshot())
        .env("STARMUX_CONFIG", &config)
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("#[range=window|0"));
    assert!(!stdout.contains("secret"));
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn context_values_that_look_like_placeholders_remain_literal() {
    let dir = temp();
    let config = dir.join("config.toml");
    fs::write(
        &config,
        "[provider.demo]\ncommand = [\"printf\", \"%s\", \"{pane.path}\"]\ncwd = \"/tmp\"\ncache = false\ndependencies = []\n",
    )
    .unwrap();
    let fields: Vec<String> = snapshot()
        .into_iter()
        .map(|field| {
            if field.starts_with("--pane-path=") {
                "--pane-path=/tmp/{session.id}".to_owned()
            } else {
                field.to_owned()
            }
        })
        .collect();
    let out = Command::new(env!("CARGO_BIN_EXE_starmux"))
        .args(["provider", "demo"])
        .args(fields)
        .env("STARMUX_CONFIG", &config)
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        "{\"demo\":{\"Plain\":\"/tmp/{session.id}\"}}\n"
    );
    fs::remove_dir_all(dir).unwrap();
}
