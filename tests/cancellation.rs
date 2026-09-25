#![cfg(unix)]

use std::{
    fs,
    path::Path,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

fn snapshot() -> Vec<&'static str> {
    vec![
        "provider",
        "demo",
        "--input-version=1",
        "--width=30",
        "--client-width=80",
        "--client-height=24",
        "--current-session=$0",
        "--current-pane=%0",
        "--pane-path=/tmp",
        "--session-count=1",
        "--session-id=$0",
        "--session-name=test",
        "--window-count=0",
        "--end-session",
    ]
}

fn start(dir: &Path, timeout_ms: u64) -> Child {
    fs::create_dir_all(dir).unwrap();
    let command = format!(
        "(sleep 0.7; printf leaked > '{}') & printf started > '{}'; wait",
        dir.join("leaked").display(),
        dir.join("started").display()
    );
    let config = format!(
        "[provider.demo]\ncommand = [\"/bin/sh\", \"-c\", {}]\ncwd = \"/tmp\"\ndecoder = \"plain\"\ntimeout_ms = {timeout_ms}\ncache = false\n",
        serde_json::to_string(&command).unwrap()
    );
    let config_path = dir.join("config.toml");
    fs::write(&config_path, config).unwrap();
    Command::new(env!("CARGO_BIN_EXE_starmux"))
        .args(snapshot())
        .env("STARMUX_CONFIG", config_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap()
}

fn wait_for_exit(child: &mut Child) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success(), "provider runner exited with {status}");
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    child.kill().unwrap();
    panic!("provider runner did not exit after cancellation");
}

#[test]
fn timeout_and_sigterm_kill_provider_descendants() {
    let root = std::env::temp_dir().join(format!("sidebar-cancellation-{}", std::process::id()));
    for (label, timeout_ms) in [("timeout", 150), ("sigterm", 3000)] {
        let dir = root.join(label);
        let mut child = start(&dir, timeout_ms);
        if label == "sigterm" {
            let deadline = Instant::now() + Duration::from_secs(2);
            while !dir.join("started").exists() && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(10));
            }
            assert!(dir.join("started").exists(), "provider did not start");
            assert_eq!(unsafe { libc::kill(child.id() as i32, libc::SIGTERM) }, 0);
        }
        wait_for_exit(&mut child);
        thread::sleep(Duration::from_millis(800));
        assert!(
            !dir.join("leaked").exists(),
            "{label} left a provider descendant running"
        );
    }
    fs::remove_dir_all(root).unwrap();
}
