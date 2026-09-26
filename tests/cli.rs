use std::process::Command;

fn binary(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_starmux"))
        .args(args)
        .env("STARMUX_CONFIG", "/dev/null")
        .output()
        .unwrap()
}

#[test]
fn help_and_adapter_expose_only_the_query_interface() {
    let help = binary(&["--help"]);
    assert!(help.status.success());
    let help = String::from_utf8(help.stdout).unwrap();
    assert!(help.contains("render-query --width=COLUMNS"));
    assert!(!help.contains("provider"));
    assert!(!help.contains("tmux-argv"));

    let adapter = binary(&["init", "tmux"]);
    assert!(adapter.status.success());
    let adapter = String::from_utf8(adapter.stdout).unwrap();
    assert!(adapter.contains("--width=#{side-status-width}"));
    assert!(adapter.contains("MouseDown1Status"));
    assert!(!adapter.contains("side-status-width 30"));
    assert!(!adapter.contains("side-status-style"));
    assert!(!adapter.contains("@window_icon"));
    assert!(!adapter.contains("@pi_win_state"));
}

#[test]
fn configuration_commands_use_portable_defaults_and_reject_old_fields() {
    let printed = binary(&["print-config"]);
    assert!(printed.status.success());
    let printed = String::from_utf8(printed.stdout).unwrap();
    assert!(printed.contains("modules = ["));
    assert!(printed.contains("\"sessions\""));
    assert!(printed.contains("character = \"-\""));
    assert!(!printed.contains("#c099ff"));
    assert!(!printed.contains("window_icon"));

    let root = std::env::temp_dir().join(format!("starmux-config-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("starmux.toml");
    std::fs::write(&path, "format = \"$sessions\"\n").unwrap();
    let checked = Command::new(env!("CARGO_BIN_EXE_starmux"))
        .arg("check-config")
        .env("STARMUX_CONFIG", &path)
        .output()
        .unwrap();
    assert!(!checked.status.success());
    assert!(String::from_utf8_lossy(&checked.stderr).contains("unknown field"));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn render_query_requires_a_valid_explicit_width_before_calling_tmux() {
    for width in ["0", "301", "nope"] {
        let output = binary(&[
            "render-query",
            &format!("--width={width}"),
            "--socket=/dev/nonexistent",
            "--client=none",
        ]);
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("width"));
    }
}

#[test]
fn version_is_reported() {
    let output = binary(&["--version"]);
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!("starmux {}\n", env!("CARGO_PKG_VERSION"))
    );
}
