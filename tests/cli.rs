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
    assert!(adapter.contains("(st|sw|sp|su|sr|sl|ss),#{mouse_status_range}"));
    assert!(adapter.contains("WheelUpStatus"));
    assert!(adapter.contains("WheelDownStatus"));
    assert!(adapter.contains("run-shell 'starmux scroll-event"));
    assert!(!adapter.contains("run-shell -b 'starmux scroll"));
    assert!(adapter.contains("(st|sw|sp|su|sr|sl|ss|sv),#{mouse_status_range}"));
    assert!(!adapter.contains("side-status-width 30"));
    assert!(!adapter.contains("side-status-style"));
    assert!(!adapter.contains("@window_icon"));
    assert!(!adapter.contains("@pi_win_state"));
}

#[test]
fn wheel_at_top_is_a_noop_without_a_tmux_refresh() {
    let cache = std::env::temp_dir().join(format!("starmux-scroll-edge-{}", std::process::id()));
    std::fs::create_dir_all(&cache).unwrap();
    let scroll = |direction| {
        Command::new(env!("CARGO_BIN_EXE_starmux"))
            .args([
                "scroll",
                "--socket=/dev/nonexistent-starmux-socket",
                "--client=missing",
                &format!("--direction={direction}"),
            ])
            .env("XDG_CACHE_HOME", &cache)
            .output()
            .unwrap()
    };
    for _ in 0..2 {
        let result = scroll("up");
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(result.stdout.is_empty());
    }
    let event = |direction| {
        Command::new(env!("CARGO_BIN_EXE_starmux"))
            .args([
                "scroll-event",
                "--socket=/dev/nonexistent-starmux-socket",
                "--client=missing",
                &format!("--direction={direction}"),
            ])
            .env("XDG_CACHE_HOME", &cache)
            .output()
            .unwrap()
    };
    assert!(event("up").status.success());
    assert!(event("down").status.success());
    assert!(event("up").status.success());
    assert!(event("up").status.success());
    let result = scroll("down");
    assert!(!result.status.success());
    assert!(result.stdout.is_empty());
    std::fs::remove_dir_all(cache).unwrap();
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

#[cfg(unix)]
#[test]
fn usage_click_opens_only_a_known_provider_page() {
    use std::os::unix::fs::PermissionsExt;

    let dir = std::env::temp_dir().join(format!("starmux-opener-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    let script = dir.join(opener);
    std::fs::write(
        &script,
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$STARMUX_TEST_ARGS\"\n",
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let args_file = dir.join("args");
    let activate = |token: &str| {
        Command::new(env!("CARGO_BIN_EXE_starmux"))
            .args([
                "activate",
                "--socket=/tmp/test.sock",
                "--client=test",
                &format!("--target={token}"),
            ])
            .env("PATH", &dir)
            .env("STARMUX_TEST_ARGS", &args_file)
            .output()
            .unwrap()
    };
    for token in [
        "su", "su2", "su3", "su5", "su7", "su8", "su4extra", "su4;echo",
    ] {
        assert!(!activate(token).status.success(), "{token}");
        assert!(!args_file.exists(), "{token}");
    }
    for (token, url) in [
        ("su0", "https://claude.ai/settings/usage"),
        (
            "su1",
            "https://github.com/settings/billing/premium_requests_usage",
        ),
        ("su4", "https://chatgpt.com/settings/usage"),
        (
            "su6",
            "https://z.ai/manage-apikey/coding-plan/personal/usage",
        ),
    ] {
        assert!(activate(token).status.success(), "{token}");
        assert_eq!(
            std::fs::read_to_string(&args_file).unwrap(),
            format!("{url}\n")
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
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

#[test]
fn named_config_is_validated_before_querying_tmux() {
    let root = std::env::temp_dir().join(format!("starmux-named-config-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("config.toml");
    std::fs::write(
        &path,
        "modules = []\n[configs.right]\nmodules = [\"divider\"]\n",
    )
    .unwrap();
    let invoke = |command: &str, config: &str| {
        Command::new(env!("CARGO_BIN_EXE_starmux"))
            .args([
                command,
                "--width=30",
                "--socket=/dev/nonexistent",
                "--client=none",
                config,
            ])
            .env("STARMUX_CONFIG", &path)
            .output()
            .unwrap()
    };
    for command in ["render-query", "explain", "timings"] {
        let output = invoke(command, "--config=missing");
        assert!(String::from_utf8_lossy(&output.stderr).contains("unknown config missing"));
        let selected = invoke(command, "--config=right");
        assert!(!String::from_utf8_lossy(&selected.stderr).contains("unknown config"));
    }
    let printed = Command::new(env!("CARGO_BIN_EXE_starmux"))
        .arg("print-config")
        .env("STARMUX_CONFIG", &path)
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&printed.stdout).contains("[configs.right]"));
    std::fs::remove_dir_all(root).unwrap();
}
