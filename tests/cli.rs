use std::process::Command;

fn binary(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_starmux"))
        .args(args)
        .env("STARMUX_CONFIG", "/dev/null")
        .output()
        .unwrap()
}

fn snapshot() -> Vec<&'static str> {
    vec![
        "--input-version=1",
        "--width=30",
        "--client-width=100",
        "--client-height=25",
        "--current-session=$0",
        "--current-pane=%0",
        "--pane-path=/tmp",
        "--session-count=2",
        "--session-id=$0",
        "--session-name=main",
        "--window-count=2",
        "--window-id=@0",
        "--window-index=0",
        "--window-name=code",
        "--selected=1",
        "--window-pane=%0",
        "--window-path=/tmp",
        "--pi-state=",
        "--window-icon=",
        "--end-window",
        "--window-id=@1",
        "--window-index=1",
        "--window-name=#[fg=red] x#{oops}",
        "--selected=0",
        "--window-pane=%1",
        "--window-path=/tmp",
        "--pi-state=working",
        "--window-icon=",
        "--end-window",
        "--end-session",
        "--session-id=$1",
        "--session-name=other",
        "--window-count=1",
        "--window-id=@2",
        "--window-index=0",
        "--window-name=notes",
        "--selected=1",
        "--window-pane=%2",
        "--window-path=/tmp",
        "--pi-state=notify",
        "--window-icon=",
        "--end-window",
        "--end-session",
    ]
}

#[test]
fn complete_snapshot_preserves_navigation_and_escapes_names() {
    let mut args = vec!["render"];
    args.extend(snapshot());
    let out = binary(&args);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let line = String::from_utf8(out.stdout).unwrap();
    assert_eq!(line.lines().count(), 1);
    assert!(line.contains("#[list=left-marker]#[acs]-#[noacs]#[nl]"));
    assert!(line.contains("#[range=session|$0"));
    assert!(line.contains("#[range=window|0 list=focus"));
    assert!(line.contains("#[range=user|sw1z141z6 "));
    assert_eq!(line.matches("#[range=window|0").count(), 1);
    assert!(line.contains("#[norange]#[list=on default]"));
    assert!(line.contains("##[fg=red] x##{oops}"));
    assert!(line.contains("#[fg=#ffc777]●"));
    assert!(line.contains("#[fg=#c099ff]●"));
    assert!(
        line.contains("#[fg=#3b4261,nobold] ────────────────────────────#[nl]"),
        "{line}"
    );
}

#[test]
fn sole_session_has_a_header_above_its_focused_window() {
    let mut args = vec!["render"];
    for field in snapshot() {
        if field == "--session-id=$1" {
            break;
        }
        args.push(if field == "--session-count=2" {
            "--session-count=1"
        } else {
            field
        });
    }
    let out = binary(&args);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let line = String::from_utf8(out.stdout).unwrap();
    let rows: Vec<_> = line.split("#[nl]").collect();
    let header = rows
        .iter()
        .position(|row| row.contains("#[range=session|$0 "))
        .expect("sole session must have a clickable header");
    assert!(
        rows[header].contains("#[fg=#1b1d2b,bg=#c099ff,bold] main "),
        "{line}"
    );
    assert!(
        rows[header + 1].contains("#[range=window|0 list=focus "),
        "focused window must follow the header: {line}"
    );
}

#[test]
fn missing_record_and_wrong_protocol_fail_visibly() {
    let mut args = vec!["render"];
    let mut fields = snapshot();
    fields.retain(|s| *s != "--end-window");
    args.extend(fields);
    let out = binary(&args);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("starmux: input error"));
    let out = binary(&["render", "--input-version=2"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("version"));
}

#[test]
fn version_and_help_identify_the_query_interface() {
    let version = binary(&["--version"]);
    assert!(version.status.success());
    assert_eq!(
        String::from_utf8(version.stdout).unwrap(),
        format!("starmux {}\n", env!("CARGO_PKG_VERSION"))
    );
    let help = binary(&["--help"]);
    assert!(help.status.success());
    assert!(
        String::from_utf8_lossy(&help.stdout).contains("render-query --socket=PATH --client=NAME")
    );
    let adapter = binary(&["init", "tmux"]);
    assert!(adapter.status.success());
    assert!(String::from_utf8_lossy(&adapter.stdout)
        .contains("--current-session=#{q/s:session_id} --current-window=#{q/s:window_id}"));
    for target in ["tmux", "tmux-argv"] {
        let generated = binary(&["init", target]);
        assert!(String::from_utf8_lossy(&generated.stdout)
            .contains("MouseDown1Status { if -F '#{m/r:^sw,#{mouse_status_range}}'"));
    }
}

#[test]
fn unreachable_tmux_socket_fails_visibly() {
    let partial = binary(&[
        "render-query",
        "--socket=/dev/nonexistent",
        "--client=/dev/nonexistent",
        "--current-session=$0",
    ]);
    assert!(!partial.status.success());
    assert!(String::from_utf8_lossy(&partial.stderr).contains("usage: render-query"));
    let malformed = binary(&[
        "render-query",
        "--socket=/dev/nonexistent",
        "--client=/dev/nonexistent",
        "--current-session=$0",
        "--current-window=invalid",
    ]);
    assert!(!malformed.status.success());
    assert!(String::from_utf8_lossy(&malformed.stderr).contains("usage: render-query"));
    let socket = format!(
        "--socket={}/sidebar-missing-{}",
        std::env::temp_dir().display(),
        std::process::id()
    );
    let out = binary(&["render-query", &socket, "--client=/dev/nonexistent"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("tmux query failed"));
    assert!(String::from_utf8_lossy(&out.stdout).contains("starmux: input error"));
}

#[test]
fn foreign_window_ids_must_fit_tmux_ids() {
    let fields: Vec<_> = snapshot()
        .into_iter()
        .map(|field| {
            if field == "--session-id=$1" {
                "--session-id=$4294967296"
            } else {
                field
            }
        })
        .collect();
    let mut args = vec!["render"];
    args.extend(fields);
    let out = binary(&args);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("invalid click target"));
}

#[test]
fn foreign_click_rejects_invalid_tokens_without_switching() {
    for token in [
        "sw",
        "sw1z141z6!",
        "sw000",
        "sw3w5e11264sgsg",
        "sw12345678901234",
    ] {
        let out = binary(&[
            "activate",
            "--socket=/dev/nonexistent",
            "--client=/dev/nonexistent",
            &format!("--target={token}"),
        ]);
        assert!(!out.status.success());
        assert!(
            String::from_utf8_lossy(&out.stderr).contains("invalid click target"),
            "{token}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let out = binary(&[
        "activate",
        "--socket=/dev/nonexistent",
        "--client=/dev/nonexistent",
        "--target=sw3w5e11264sgsf",
    ]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("tmux switch failed"));
}

#[test]
fn unreferenced_provider_does_not_execute() {
    let root = std::env::temp_dir().join(format!("starmux-unused-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let marker = root.join("ran");
    let config = root.join("config.toml");
    std::fs::write(
        &config,
        format!(
            "[provider.unused]\ncommand = [\"/usr/bin/touch\", \"{}\"]\ncwd = \"/tmp\"\n",
            marker.display()
        ),
    )
    .unwrap();
    let mut args = vec!["render"];
    args.extend(snapshot());
    let out = Command::new(env!("CARGO_BIN_EXE_starmux"))
        .args(&args)
        .env("STARMUX_CONFIG", &config)
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(!marker.exists());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn default_config_has_no_providers() {
    let out = binary(&["print-config"]);
    assert!(out.status.success());
    let config = String::from_utf8(out.stdout).unwrap();
    assert_eq!(config, "format = \"$sessions$divider\"\n");
}
