#[cfg(any(target_os = "macos", target_os = "linux"))]
fn attached_client(socket: &str, target: &str) -> std::process::Command {
    let mut command = std::process::Command::new("script");
    #[cfg(target_os = "macos")]
    command.args([
        "-q",
        "/dev/null",
        "tmux",
        "-L",
        socket,
        "attach",
        "-t",
        target,
    ]);
    #[cfg(target_os = "linux")]
    command.args([
        "-q",
        "-c",
        &format!("tmux -L {socket} attach -t {target}"),
        "/dev/null",
    ]);
    command
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[test]
fn gob_failures_leave_other_rows_and_diagnostics_available() {
    use std::{
        fs,
        os::unix::fs::PermissionsExt,
        process::{Command, Stdio},
        thread,
        time::Duration,
    };

    let root = std::env::temp_dir().join(format!("starmux-gob-failure-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let socket = format!("starmux-gob-failure-{}", std::process::id());
    let gob = root.join("gob");
    let config = root.join("config.toml");
    fs::write(
        &config,
        "modules = [\"sessions\", \"gob\", \"command.good\", \"command.bad\"]\n[commands.good]\nargv = [\"/bin/echo\", \"healthy\"]\n[commands.bad]\nargv = [\"/usr/bin/false\"]\n",
    )
    .unwrap();
    let tmux = |args: &[&str]| {
        let output = Command::new("tmux")
            .args(["-L", &socket])
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        output
    };
    tmux(&[
        "-f",
        "/dev/null",
        "new-session",
        "-d",
        "-s",
        "main",
        "-c",
        root.to_str().unwrap(),
        "sleep 15",
    ]);
    let mut attached = attached_client(&socket, "main")
        .env("TERM", "xterm-256color")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let _input = attached.stdin.take();
    let mut client = String::new();
    for _ in 0..30 {
        client = String::from_utf8(tmux(&["list-clients", "-F", "#{client_name}"]).stdout)
            .unwrap_or_default();
        if !client.trim().is_empty() {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    assert!(!client.trim().is_empty());
    let socket_path =
        String::from_utf8(tmux(&["display-message", "-p", "#{socket_path}"]).stdout).unwrap();
    let binary = env!("CARGO_BIN_EXE_starmux");
    let path = format!("{}:{}", root.display(), std::env::var("PATH").unwrap());
    let query = |command: &str| {
        Command::new(binary)
            .args([
                command,
                "--width=30",
                &format!("--socket={}", socket_path.trim()),
                &format!("--client={}", client.trim()),
            ])
            .env("STARMUX_CONFIG", &config)
            .env("PATH", &path)
            .output()
            .unwrap()
    };
    for (script, expected) in [
        ("#!/bin/sh\n/bin/sleep 2\n", "gob list timed out"),
        ("#!/bin/sh\nprintf 'invalid json'\n", "invalid gob list"),
        (
            "#!/bin/sh\nprintf 'unavailable' >&2\nexit 7\n",
            "gob list failed: unavailable",
        ),
    ] {
        fs::write(&gob, script).unwrap();
        fs::set_permissions(&gob, fs::Permissions::from_mode(0o700)).unwrap();
        let rendered = query("render-query");
        assert!(
            rendered.status.success(),
            "{}",
            String::from_utf8_lossy(&rendered.stderr)
        );
        let text = String::from_utf8(rendered.stdout).unwrap();
        assert!(text.contains("main") && text.contains("healthy"), "{text}");
        assert!(
            !text.contains(" Jobs") && !text.contains("input error"),
            "{text}"
        );
        let explained = query("explain");
        assert!(
            explained.status.success(),
            "{}",
            String::from_utf8_lossy(&explained.stderr)
        );
        let text = String::from_utf8(explained.stdout).unwrap();
        assert!(text.contains(&format!("gob: {expected}")), "{text}");
        assert!(text.contains("command.bad: command exited"), "{text}");
        assert!(text.contains("command.good output=\"healthy\""), "{text}");
    }
    let timings = query("timings");
    assert!(
        timings.status.success(),
        "{}",
        String::from_utf8_lossy(&timings.stderr)
    );
    let metrics: serde_json::Value = serde_json::from_slice(&timings.stdout).unwrap();
    assert!(metrics["gob_us"].as_u64().unwrap() > 0);

    tmux(&["kill-server"]);
    attached.wait().unwrap();
    fs::remove_dir_all(root).unwrap();
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[test]
fn attached_side_status_paints_navigation_rows() {
    use std::{
        fs,
        process::{Command, Stdio},
        thread,
        time::Duration,
    };
    let root = std::env::temp_dir().join(format!("starmux-live-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let socket = format!("starmux-test-{}", std::process::id());
    let binary = env!("CARGO_BIN_EXE_starmux");
    let config_path = root.join("config.toml");
    let generated = Command::new(binary)
        .args(["init", "tmux"])
        .output()
        .unwrap();
    assert!(generated.status.success());
    let adapter = String::from_utf8(generated.stdout)
        .unwrap()
        .replace(
            "starmux render-query",
            &format!(
                "env STARMUX_CONFIG={} {} render-query",
                config_path.display(),
                binary
            ),
        )
        .replace(")'", &format!(" 2>>{})'", root.join("job.err").display()));
    let adapter_path = root.join("adapter.conf");
    fs::write(&adapter_path, adapter).unwrap();
    fs::write(
        &config_path,
        "[sessions]\nwindow_format = \" $index $indicator $name\"\n[sessions.window_options]\nstate = \"@agent_state\"\n[sessions.indicator]\nfallback = \".\"\n[[sessions.indicator.rules]]\nwhen = { state = \"working\" }\ntext = \"*\"\n",
    )
    .unwrap();
    let tmux = |args: &[&str]| {
        let output = Command::new("tmux")
            .args(["-L", &socket])
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "tmux {:?}: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    };
    tmux(&[
        "-f",
        "/dev/null",
        "new-session",
        "-d",
        "-s",
        "main",
        "-n",
        "alpha",
        "sleep 10",
    ]);
    let supported = Command::new("tmux")
        .args(["-L", &socket, "show-options", "-gv", "side-status"])
        .output()
        .unwrap()
        .status
        .success();
    if !supported {
        tmux(&["kill-server"]);
        fs::remove_dir_all(root).unwrap();
        assert_ne!(
            std::env::var_os("STARMUX_REQUIRE_SIDE_STATUS"),
            Some("1".into()),
            "tmux lacks side-status; the release parity gate cannot run"
        );
        eprintln!("skipping attached-client test: tmux lacks side-status");
        return;
    }
    tmux(&["new-window", "-d", "-t", "main:", "-n", "beta", "sleep 10"]);
    tmux(&[
        "set-option",
        "-w",
        "-t",
        "main:1",
        "@agent_state",
        "working",
    ]);
    tmux(&[
        "new-window",
        "-d",
        "-t",
        "main:",
        "-n",
        "a'b$#[]",
        "sleep 10",
    ]);
    tmux(&["new-session", "-d", "-s", "aux", "-n", "gamma", "sleep 10"]);
    tmux(&[
        "set-environment",
        "-g",
        "STARMUX_CONFIG",
        config_path.to_str().unwrap(),
    ]);
    tmux(&["set", "-g", "status-interval", "1"]);
    tmux(&["set", "-g", "side-status", "left"]);
    tmux(&["set", "-g", "side-status-width", "30"]);
    tmux(&["source-file", adapter_path.to_str().unwrap()]);
    let capture = root.join("client.out");
    let output = fs::File::create(&capture).unwrap();
    let mut client = attached_client(&socket, "main")
        .env("TERM", "xterm-256color")
        .stdin(Stdio::piped())
        .stdout(Stdio::from(output))
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let _input = client.stdin.take();
    let mut first = String::new();
    for _ in 0..35 {
        thread::sleep(Duration::from_millis(100));
        first = fs::read_to_string(&capture).unwrap();
        if first.contains("alpha") && first.contains("beta") && first.contains("gamma") {
            break;
        }
    }
    assert!(
        first.contains("alpha") && first.contains("beta") && first.contains("gamma"),
        "first paint missing; job errors: {:?}",
        fs::read_to_string(root.join("job.err"))
    );
    assert!(
        first.contains("a'b$#[]"),
        "literal window name was changed by tmux expansion"
    );
    assert!(
        first.contains("* beta"),
        "configured option was not rendered"
    );
    let client_name = String::from_utf8(
        Command::new("tmux")
            .args(["-L", &socket, "list-clients", "-F", "#{client_name}"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    let focus = Command::new("tmux")
        .args([
            "-L",
            &socket,
            "display-message",
            "-p",
            "-c",
            client_name.trim(),
            "#{socket_path}|#{session_id}|#{window_id}",
        ])
        .output()
        .unwrap();
    assert!(focus.status.success());
    let fields = String::from_utf8(focus.stdout).unwrap();
    let parts: Vec<_> = fields.trim().split('|').collect();
    assert_eq!(parts.len(), 3);
    let query = [
        "render-query".to_string(),
        "--width=30".to_string(),
        format!("--socket={}", parts[0]),
        format!("--client={}", client_name.trim()),
        format!("--current-session={}", parts[1]),
        format!("--current-window={}", parts[2]),
    ];
    let valid = Command::new(binary)
        .args(&query)
        .env("STARMUX_CONFIG", &config_path)
        .output()
        .unwrap();
    assert!(
        valid.status.success(),
        "{}",
        String::from_utf8_lossy(&valid.stderr)
    );
    assert!(String::from_utf8_lossy(&valid.stdout).contains("#[range=user|sw"));
    tmux(&["switch-client", "-c", client_name.trim(), "-t", "aux"]);
    let stale = Command::new(binary)
        .args(&query)
        .env("STARMUX_CONFIG", &config_path)
        .output()
        .unwrap();
    assert!(stale.status.success());
    assert!(stale.stderr.is_empty());
    assert!(String::from_utf8_lossy(&stale.stdout).trim().is_empty());
    let explained = Command::new(binary)
        .arg("explain")
        .args(&query[1..])
        .env("STARMUX_CONFIG", &config_path)
        .output()
        .unwrap();
    assert!(!explained.status.success());
    assert!(String::from_utf8_lossy(&explained.stderr).contains("tmux focus changed during query"));
    let current = Command::new("tmux")
        .args([
            "-L",
            &socket,
            "display-message",
            "-p",
            "-c",
            client_name.trim(),
            "#{session_id}|#{window_id}",
        ])
        .output()
        .unwrap();
    assert!(current.status.success());
    let focus: Vec<_> = String::from_utf8(current.stdout)
        .unwrap()
        .trim()
        .split('|')
        .map(str::to_owned)
        .collect();
    let mut fresh = query.clone();
    fresh[4] = format!("--current-session={}", focus[0]);
    fresh[5] = format!("--current-window={}", focus[1]);
    let rendered = Command::new(binary)
        .args(&fresh)
        .env("STARMUX_CONFIG", &config_path)
        .output()
        .unwrap();
    assert!(rendered.status.success());
    assert!(String::from_utf8_lossy(&rendered.stdout).contains("gamma"));
    tmux(&["switch-client", "-c", client_name.trim(), "-t", "main"]);

    fs::write(&config_path, "modules = [\"spacer\", \"divider\"]\n").unwrap();
    for (status, expected_status_rows) in [("on", 1), ("off", 0), ("2", 2)] {
        tmux(&["set", "-g", "status", status]);
        let height = Command::new("tmux")
            .args([
                "-L",
                &socket,
                "display-message",
                "-p",
                "-c",
                client_name.trim(),
                "#{client_height}",
            ])
            .output()
            .unwrap();
        assert!(height.status.success());
        let height: usize = String::from_utf8(height.stdout)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        let padded = Command::new(binary)
            .args(&query)
            .env("STARMUX_CONFIG", &config_path)
            .output()
            .unwrap();
        assert!(
            padded.status.success(),
            "{}",
            String::from_utf8_lossy(&padded.stderr)
        );
        let padded = String::from_utf8(padded.stdout).unwrap();
        assert_eq!(
            padded.matches("#[nl]").count(),
            height - expected_status_rows + 2
        );
        assert!(padded.contains("----------------------------"));
    }
    let rows = std::iter::once("\"divider\"")
        .chain(std::iter::repeat_n("\"blank\"", 80))
        .collect::<Vec<_>>()
        .join(", ");
    fs::write(
        &config_path,
        format!("modules = [{rows}]\n[configs.right]\nmodules = [{rows}]\n"),
    )
    .unwrap();
    let named = Command::new(binary)
        .args(&query)
        .arg("--config=right")
        .env("STARMUX_CONFIG", &config_path)
        .output()
        .unwrap();
    assert!(named.status.success());
    assert!(String::from_utf8_lossy(&named.stdout).contains("----------------------------"));
    let wheel = Command::new(binary)
        .args([
            "scroll-event",
            &format!("--socket={}", parts[0]),
            &format!("--client={}", client_name.trim()),
            "--direction=down",
            "--config=right",
        ])
        .env("STARMUX_CONFIG", &config_path)
        .output()
        .unwrap();
    assert!(
        wheel.status.success(),
        "{}",
        String::from_utf8_lossy(&wheel.stderr)
    );
    let shifted = Command::new(binary)
        .args(&query)
        .arg("--config=right")
        .env("STARMUX_CONFIG", &config_path)
        .output()
        .unwrap();
    assert!(shifted.status.success());
    assert!(!String::from_utf8_lossy(&shifted.stdout).contains("----------------------------"));
    let default = Command::new(binary)
        .args(&query)
        .env("STARMUX_CONFIG", &config_path)
        .output()
        .unwrap();
    assert!(default.status.success());
    assert!(String::from_utf8_lossy(&default.stdout).contains("----------------------------"));
    tmux(&["kill-server"]);
    let _ = client.wait();
    fs::remove_dir_all(root).unwrap();
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[test]
fn clicking_foreign_window_switches_the_attached_client_to_that_window() {
    use std::{
        fs,
        io::Write,
        process::{Command, Stdio},
        thread,
        time::Duration,
    };

    let root = std::env::temp_dir().join(format!("starmux-click-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let socket = format!("starmux-click-{}", std::process::id());
    let binary = env!("CARGO_BIN_EXE_starmux");
    let generated = Command::new(binary)
        .args(["init", "tmux"])
        .output()
        .unwrap();
    assert!(generated.status.success());
    let adapter = String::from_utf8(generated.stdout)
        .unwrap()
        .replace(
            "starmux render-query",
            &format!("env STARMUX_CONFIG=/dev/null {binary} render-query"),
        )
        .replace("starmux activate", &format!("{binary} activate"));
    let adapter_path = root.join("adapter.conf");
    fs::write(&adapter_path, adapter).unwrap();

    let tmux = |args: &[&str]| -> String {
        let output = Command::new("tmux")
            .args(["-L", &socket])
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "tmux {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    };
    tmux(&[
        "-f",
        "/dev/null",
        "new-session",
        "-d",
        "-s",
        "main",
        "-n",
        "alpha",
        "sleep 60",
    ]);
    let supported = Command::new("tmux")
        .args(["-L", &socket, "show-options", "-gv", "side-status"])
        .output()
        .unwrap()
        .status
        .success();
    if !supported {
        tmux(&["kill-server"]);
        fs::remove_dir_all(root).unwrap();
        assert_ne!(
            std::env::var_os("STARMUX_REQUIRE_SIDE_STATUS"),
            Some("1".into()),
            "tmux lacks side-status; the click parity gate cannot run"
        );
        eprintln!("skipping attached-client click test: tmux lacks side-status");
        return;
    }
    tmux(&["new-window", "-d", "-t", "main:", "-n", "beta", "sleep 60"]);
    tmux(&["new-session", "-d", "-s", "aux", "-n", "gamma", "sleep 60"]);
    tmux(&["new-window", "-d", "-t", "aux:", "-n", "delta", "sleep 60"]);
    tmux(&["set", "-g", "mouse", "on"]);
    tmux(&["set", "-g", "status-interval", "1"]);
    tmux(&["set", "-g", "side-status", "left"]);
    tmux(&["set", "-g", "side-status-width", "30"]);
    tmux(&["source-file", adapter_path.to_str().unwrap()]);
    let capture = root.join("client.out");
    let mut client = attached_client(&socket, "main")
        .env("TERM", "xterm-256color")
        .stdin(Stdio::piped())
        .stdout(Stdio::from(fs::File::create(&capture).unwrap()))
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut input = client.stdin.take().unwrap();
    let mut client_name = String::new();
    for _ in 0..35 {
        client_name = tmux(&["list-clients", "-F", "#{client_name}"]);
        if !client_name.is_empty() && fs::read_to_string(&capture).unwrap().contains("delta") {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    assert!(!client_name.is_empty() && fs::read_to_string(&capture).unwrap().contains("delta"));
    let focus = || {
        tmux(&[
            "display-message",
            "-p",
            "-c",
            &client_name,
            "#{session_id}|#{window_id}",
        ])
    };
    let target = |window: &str| {
        tmux(&[
            "display-message",
            "-p",
            "-t",
            window,
            "#{session_id}|#{window_id}",
        ])
    };
    let click = |x: u8, y: u8, input: &mut std::process::ChildStdin| {
        write!(input, "\x1b[<0;{x};{y}M\x1b[<0;{x};{y}m").unwrap();
        input.flush().unwrap();
    };
    let expect_focus = |expected: &str| {
        for _ in 0..30 {
            if focus() == expected {
                return;
            }
            thread::sleep(Duration::from_millis(100));
        }
        panic!("expected {expected}, got {}", focus());
    };
    assert_eq!(focus(), target("main:0"));
    click(30, 6, &mut input);
    thread::sleep(Duration::from_millis(350));
    assert_eq!(focus(), target("main:0"));
    click(29, 7, &mut input);
    thread::sleep(Duration::from_millis(350));
    assert_eq!(focus(), target("main:0"));
    click(29, 6, &mut input);
    expect_focus(&target("aux:1"));
    thread::sleep(Duration::from_millis(600));
    click(29, 5, &mut input);
    expect_focus(&target("aux:0"));
    thread::sleep(Duration::from_millis(600));
    click(29, 1, &mut input);
    expect_focus(&target("main:0"));
    thread::sleep(Duration::from_millis(600));
    click(29, 3, &mut input);
    expect_focus(&target("main:1"));

    tmux(&["link-window", "-s", "main:0", "-t", "aux:2"]);
    for _ in 0..30 {
        if fs::read_to_string(&capture).unwrap().contains("2: alpha") {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    assert!(fs::read_to_string(&capture).unwrap().contains("2: alpha"));
    thread::sleep(Duration::from_millis(600));
    click(29, 7, &mut input);
    expect_focus(&target("aux:2"));
    tmux(&["switch-client", "-c", &client_name, "-t", "main:1"]);
    expect_focus(&target("main:1"));

    let socket_path = tmux(&[
        "display-message",
        "-p",
        "-c",
        &client_name,
        "#{socket_path}",
    ]);
    let rendered = Command::new(binary)
        .args([
            "render-query",
            "--width=30",
            &format!("--socket={socket_path}"),
            &format!("--client={client_name}"),
        ])
        .env("STARMUX_CONFIG", "/dev/null")
        .output()
        .unwrap();
    assert!(rendered.status.success());
    let rendered = String::from_utf8(rendered.stdout).unwrap();
    let row = rendered
        .split("#[nl]")
        .find(|row| row.contains("delta"))
        .unwrap();
    let token = row
        .split("#[range=user|")
        .nth(1)
        .unwrap()
        .split(' ')
        .next()
        .unwrap();
    tmux(&["kill-window", "-t", "aux:1"]);
    let previous = focus();
    let stale = Command::new(binary)
        .args([
            "activate",
            &format!("--socket={socket_path}"),
            &format!("--client={client_name}"),
            &format!("--target={token}"),
        ])
        .output()
        .unwrap();
    assert!(!stale.status.success());
    assert_eq!(focus(), previous);

    tmux(&["kill-server"]);
    let _ = client.wait();
    fs::remove_dir_all(root).unwrap();
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[test]
fn clicking_usage_opens_the_provider_page() {
    use std::{
        fs,
        io::Write,
        os::unix::fs::PermissionsExt,
        process::{Command, Stdio},
        thread,
        time::Duration,
    };

    let root = std::env::temp_dir().join(format!("starmux-usage-click-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let socket = format!("starmux-usage-click-{}", std::process::id());
    let binary = env!("CARGO_BIN_EXE_starmux");
    let opener = root.join(if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    });
    fs::write(
        &opener,
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$STARMUX_TEST_ARGS\"\n",
    )
    .unwrap();
    fs::set_permissions(&opener, fs::Permissions::from_mode(0o755)).unwrap();
    let args_file = root.join("opened");
    let config = root.join("config.toml");
    fs::write(
        &config,
        format!(
            "modules = [\"usage\"]\n[usage]\nproviders = [\"codex\"]\ncache_dir = {:?}\n",
            root.join("cache")
        ),
    )
    .unwrap();
    let generated = Command::new(binary)
        .args(["init", "tmux"])
        .output()
        .unwrap();
    assert!(generated.status.success());
    let adapter = String::from_utf8(generated.stdout)
        .unwrap()
        .replace(
            "starmux render-query",
            &format!(
                "env STARMUX_CONFIG={} {binary} render-query",
                config.display()
            ),
        )
        .replace(
            "starmux activate",
            &format!(
                "env PATH={} STARMUX_TEST_ARGS={} {binary} activate",
                root.display(),
                args_file.display()
            ),
        );
    let adapter_path = root.join("adapter.conf");
    fs::write(&adapter_path, adapter).unwrap();
    let tmux = |args: &[&str]| {
        let output = Command::new("tmux")
            .args(["-L", &socket])
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "tmux {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    tmux(&[
        "-f",
        "/dev/null",
        "new-session",
        "-d",
        "-s",
        "main",
        "sleep 60",
    ]);
    let supported = Command::new("tmux")
        .args(["-L", &socket, "show-options", "-gv", "side-status"])
        .output()
        .unwrap()
        .status
        .success();
    if !supported {
        tmux(&["kill-server"]);
        fs::remove_dir_all(root).unwrap();
        assert_ne!(
            std::env::var_os("STARMUX_REQUIRE_SIDE_STATUS"),
            Some("1".into())
        );
        return;
    }
    tmux(&["set", "-g", "mouse", "on"]);
    tmux(&["set", "-g", "status-interval", "1"]);
    tmux(&["set", "-g", "side-status", "left"]);
    tmux(&["set", "-g", "side-status-width", "30"]);
    tmux(&["source-file", adapter_path.to_str().unwrap()]);
    let capture = root.join("client.out");
    let mut client = attached_client(&socket, "main")
        .env("TERM", "xterm-256color")
        .stdin(Stdio::piped())
        .stdout(Stdio::from(fs::File::create(&capture).unwrap()))
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut input = client.stdin.take().unwrap();
    for _ in 0..40 {
        if fs::read_to_string(&capture).unwrap().contains("codex") {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    assert!(fs::read_to_string(&capture).unwrap().contains("codex"));
    write!(input, "\x1b[<0;3;1M\x1b[<0;3;1m").unwrap();
    input.flush().unwrap();
    for _ in 0..40 {
        if args_file.exists() {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    assert_eq!(
        fs::read_to_string(&args_file).unwrap(),
        "https://chatgpt.com/settings/usage\n"
    );
    tmux(&["kill-server"]);
    let _ = client.wait();
    fs::remove_dir_all(root).unwrap();
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[test]
fn pi_attention_row_click_selects_its_pane() {
    use std::{
        fs,
        io::{Read, Write},
        os::unix::net::UnixListener,
        process::{Command, Stdio},
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
        thread,
        time::Duration,
    };
    let root = std::env::temp_dir().join(format!("starmux-pi-click-{}", std::process::id()));
    fs::create_dir_all(root.join("status")).unwrap();
    fs::create_dir_all(root.join("sockets")).unwrap();
    let socket = format!("starmux-pi-click-{}", std::process::id());
    let binary = env!("CARGO_BIN_EXE_starmux");
    let tmux = |args: &[&str]| -> String {
        let output = Command::new("tmux")
            .args(["-L", &socket])
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "tmux {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    };
    tmux(&[
        "-f",
        "/dev/null",
        "new-session",
        "-d",
        "-s",
        "main",
        "-n",
        "alpha",
        "sleep 60",
    ]);
    let supported = Command::new("tmux")
        .args(["-L", &socket, "show-options", "-gv", "side-status"])
        .output()
        .unwrap()
        .status
        .success();
    if !supported {
        tmux(&["kill-server"]);
        fs::remove_dir_all(root).unwrap();
        assert_ne!(
            std::env::var_os("STARMUX_REQUIRE_SIDE_STATUS"),
            Some("1".into()),
            "tmux lacks side-status"
        );
        return;
    }
    let server_socket = tmux(&["display-message", "-p", "-t", "main:0", "#{socket_path}"]);
    let working = tmux(&["display-message", "-p", "-t", "main:0", "#{pane_id}"]);
    let foreign_socket = format!("{socket}-foreign");
    let foreign_tmux = |args: &[&str]| -> String {
        let output = Command::new("tmux")
            .args(["-L", &foreign_socket])
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    };
    foreign_tmux(&[
        "-f",
        "/dev/null",
        "new-session",
        "-d",
        "-s",
        "main",
        "sleep 60",
    ]);
    let foreign_pane = foreign_tmux(&["display-message", "-p", "-t", "main:0", "#{pane_id}"]);
    let foreign_server_socket =
        foreign_tmux(&["display-message", "-p", "-t", "main:0", "#{socket_path}"]);
    assert_eq!(working, foreign_pane);
    tmux(&["new-window", "-d", "-t", "main:", "-n", "beta", "sleep 60"]);
    let attention = tmux(&[
        "split-window",
        "-d",
        "-t",
        "main:1",
        "-P",
        "-F",
        "#{pane_id}",
        "sleep 60",
    ]);
    tmux(&["set-option", "-p", "-t", &attention, "@pi_state", "notify"]);
    tmux(&["set-option", "-p", "-t", &working, "@pi_state", "working"]);
    let pi_socket = root.join("sockets/pi.sock");
    let listener = UnixListener::bind(&pi_socket).unwrap();
    listener.set_nonblocking(true).unwrap();
    let running = Arc::new(AtomicBool::new(true));
    let listening = running.clone();
    let responder = thread::spawn(move || {
        while listening.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let mut request = [0; 128];
                    if stream.read(&mut request).is_ok_and(|count| count > 0) {
                        let _ = stream.write_all(b"{\"ok\":true,\"result\":{\"type\":\"pong\"}}\n");
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10))
                }
                Err(error) => panic!("Pi test socket: {error}"),
            }
        }
    });
    for (id, name, pane, state) in [
        ("attention", "attention", &attention, "idle"),
        ("working", "working item", &working, "working"),
    ] {
        let session_file = root.join(format!("{id}.jsonl"));
        let context_path = root.join(format!("{id}.jsonl.context.json"));
        let plan_relative = format!("{id}.jsonl.plans/0123456789abcdef01234567.md");
        let plan_path = root.join(&plan_relative);
        fs::create_dir_all(plan_path.parent().unwrap()).unwrap();
        fs::write(&plan_path, format!("# {id} plan")).unwrap();
        let skill_path = root.join(format!("skills/{id}/SKILL.md"));
        fs::create_dir_all(skill_path.parent().unwrap()).unwrap();
        fs::write(&skill_path, "# Testing").unwrap();
        fs::write(&context_path, serde_json::json!({
            "version": 2, "sessionId": id, "extensions": {
                "pi-plans": {"version": 1, "data": {"plans": [{"id": "0123456789abcdef01234567", "title": format!("{id} plan"), "path": plan_relative}]}},
                "pi-skills": {"version": 1, "data": {"skills": ["testing"], "skillPaths": {"testing": skill_path}}}
            }
        }).to_string()).unwrap();
        let status = serde_json::json!({
            "version": 2, "sessionId": id, "name": name, "pid": 1,
            "cwd": "/tmp", "sessionFile": session_file,
            "contextPath": context_path, "startedAt": "2026-01-01T00:00:00Z",
            "updatedAt": "2026-01-01T00:00:00Z", "state": state,
            "extensions": {
                "pi-socket": {"version": 1, "data": {"socketPath": pi_socket}},
                "pi-tmux": {"version": 1, "data": {"paneId": pane, "sessionName": "main", "windowIndex": 0, "windowName": "old", "socketPath": server_socket}}
            }
        });
        fs::write(
            root.join("status").join(format!("{id}.json")),
            status.to_string(),
        )
        .unwrap();
    }
    fs::write(
        root.join("status/foreign.json"),
        serde_json::json!({
            "version": 2, "sessionId": "foreign", "name": "foreign pi", "pid": 1,
            "cwd": "/tmp",
            "startedAt": "2026-01-01T00:00:00Z", "updatedAt": "2026-01-01T00:00:00Z",
            "state": "idle", "extensions": {
                "pi-socket": {"version": 1, "data": {"socketPath": pi_socket}},
                "pi-tmux": {"version": 1, "data": {"paneId": foreign_pane, "sessionName": "main", "socketPath": foreign_server_socket}}
            }
        }).to_string(),
    ).unwrap();
    let config = root.join("config.toml");
    fs::write(
        &config,
        format!(
            "modules = [\"sessions\", \"divider\", \"pi-workbench\", \"divider\", \"pi-context\"]\n[pi-workbench]\ndata_dir = {:?}\n[pi-context]\nopen_command = [\"dev\", \"tmux\", \"edit\", \"{{file}}\", \"{{pane}}\", \"{{socket}}\"]\n",
            root.to_str().unwrap()
        ),
    )
    .unwrap();
    let adapter = String::from_utf8(
        Command::new(binary)
            .args(["init", "tmux"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .replace(
        "starmux render-query",
        &format!(
            "env STARMUX_CONFIG={} {binary} render-query",
            config.display()
        ),
    )
    .replace("starmux activate", &format!("{binary} activate"));
    let adapter_path = root.join("adapter.conf");
    fs::write(&adapter_path, adapter).unwrap();
    tmux(&["set", "-g", "mouse", "on"]);
    tmux(&["set", "-g", "status-interval", "1"]);
    tmux(&["set", "-g", "side-status", "left"]);
    tmux(&["set", "-g", "side-status-width", "30"]);
    tmux(&["source-file", adapter_path.to_str().unwrap()]);
    let capture = root.join("client.out");
    let mut client = attached_client(&socket, "main")
        .env("TERM", "xterm-256color")
        .stdin(Stdio::piped())
        .stdout(Stdio::from(fs::File::create(&capture).unwrap()))
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut input = client.stdin.take().unwrap();
    let mut client_name = String::new();
    for _ in 0..40 {
        client_name = tmux(&["list-clients", "-F", "#{client_name}"]);
        if !client_name.is_empty() && fs::read_to_string(&capture).unwrap().contains("attention") {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    assert!(!client_name.is_empty() && fs::read_to_string(&capture).unwrap().contains("attention"));
    let focus = || {
        tmux(&[
            "display-message",
            "-p",
            "-c",
            &client_name,
            "#{window_id}|#{pane_id}",
        ])
    };
    let socket_path = tmux(&[
        "display-message",
        "-p",
        "-c",
        &client_name,
        "#{socket_path}",
    ]);
    let mut rendered = String::new();
    for _ in 0..10 {
        let output = Command::new(binary)
            .args([
                "render-query",
                "--width=30",
                &format!("--socket={socket_path}"),
                &format!("--client={client_name}"),
            ])
            .env("STARMUX_CONFIG", &config)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        rendered = String::from_utf8(output.stdout).unwrap();
        if rendered.contains("attention") && rendered.contains("working item") {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    assert!(!rendered.contains("foreign pi"), "{rendered}");
    assert!(
        rendered.find("attention").is_some_and(|position| rendered
            .find("working item")
            .is_some_and(|other| position < other)),
        "{rendered}"
    );
    let row = rendered
        .split("#[nl]")
        .find(|row| row.contains("attention"))
        .unwrap();
    assert!(row.contains("#[fg=magenta]●"), "{row}");
    assert!(row.contains("#[range=user|sp"), "{row}");
    write!(input, "\x1b[<0;29;6M\x1b[<0;29;6m").unwrap();
    input.flush().unwrap();
    let target_window = tmux(&["display-message", "-p", "-t", &attention, "#{window_id}"]);
    for _ in 0..30 {
        if focus() == format!("{target_window}|{attention}") {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    assert_eq!(focus(), format!("{target_window}|{attention}"));
    let mut selected_context = String::new();
    for _ in 0..10 {
        let output = Command::new(binary)
            .args([
                "render-query",
                "--width=30",
                &format!("--socket={socket_path}"),
                &format!("--client={client_name}"),
            ])
            .env("STARMUX_CONFIG", &config)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        selected_context = String::from_utf8(output.stdout).unwrap();
        if selected_context.contains("attention plan") && selected_context.contains("✦") {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    assert!(
        selected_context.contains("attention plan") && selected_context.contains("✦"),
        "{selected_context}"
    );
    assert!(
        !selected_context.contains("working plan"),
        "{selected_context}"
    );
    use std::os::unix::fs::PermissionsExt;
    let opener = root.join("dev");
    fs::write(
        &opener,
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$STARMUX_TEST_ARGS\"\n",
    )
    .unwrap();
    fs::set_permissions(&opener, fs::Permissions::from_mode(0o700)).unwrap();
    let opened = root.join("opened-file");
    for (kind, expected) in [
        (
            "sl",
            root.join("attention.jsonl.plans/0123456789abcdef01234567.md"),
        ),
        ("ss", root.join("skills/attention/SKILL.md")),
    ] {
        let token = selected_context
            .split(&format!("#[range=user|{kind}"))
            .nth(1)
            .unwrap()
            .split(' ')
            .next()
            .unwrap();
        let target = format!("--target={kind}{token}");
        let output = Command::new(binary)
            .args([
                "activate",
                &format!("--socket={socket_path}"),
                &format!("--client={client_name}"),
                &target,
            ])
            .env("STARMUX_CONFIG", &config)
            .env("STARMUX_TEST_ARGS", &opened)
            .env(
                "PATH",
                format!("{}:{}", root.display(), std::env::var("PATH").unwrap()),
            )
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "kind={kind} target={target} expected={} focus={} context={selected_context}: {}",
            expected.display(),
            focus(),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            fs::read_to_string(&opened).unwrap().trim(),
            format!(
                "tmux\nedit\n{}\n{}\n{}",
                expected.display(),
                attention,
                socket_path
            )
        );
    }
    let token = row
        .split("#[range=user|")
        .nth(1)
        .unwrap()
        .split(' ')
        .next()
        .unwrap();
    tmux(&["kill-pane", "-t", &attention]);
    let focus_after_close = focus();
    let stale = Command::new(binary)
        .args([
            "activate",
            &format!("--socket={socket_path}"),
            &format!("--client={client_name}"),
            &format!("--target={token}"),
        ])
        .output()
        .unwrap();
    assert!(!stale.status.success());
    assert_eq!(focus(), focus_after_close);
    tmux(&["kill-server"]);
    foreign_tmux(&["kill-server"]);
    let _ = client.wait();
    running.store(false, Ordering::Relaxed);
    responder.join().unwrap();
    fs::remove_dir_all(root).unwrap();
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[test]
fn wheel_scrolls_all_sidebar_rows_without_switching_windows() {
    use std::{
        fs,
        io::Write,
        process::{Command, Stdio},
        thread,
        time::Duration,
    };

    let root = std::env::temp_dir().join(format!("starmux-wheel-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let socket = format!("starmux-wheel-{}", std::process::id());
    let binary = env!("CARGO_BIN_EXE_starmux");
    let config = root.join("config.toml");
    fs::write(
        &config,
        "modules = [\"sessions\", \"divider\", \"blank\", \"command.slow\"]\n[commands.slow]\nargv = [\"sleep\", \"0.07\"]\n",
    )
    .unwrap();
    let generated = Command::new(binary)
        .args(["init", "tmux"])
        .output()
        .unwrap();
    assert!(generated.status.success());
    let adapter = String::from_utf8(generated.stdout)
        .unwrap()
        .replace("starmux render-query", &format!("{binary} render-query"))
        .replace("starmux scroll", &format!("{binary} scroll"))
        .replace("starmux activate", &format!("{binary} activate"));
    let adapter_path = root.join("adapter.conf");
    fs::write(&adapter_path, adapter).unwrap();
    let tmux = |args: &[&str]| {
        let output = Command::new("tmux")
            .args(["-L", &socket])
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "tmux {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    };
    tmux(&[
        "-f",
        "/dev/null",
        "new-session",
        "-d",
        "-s",
        "main",
        "-n",
        "first",
        "sleep 60",
    ]);
    let supported = Command::new("tmux")
        .args(["-L", &socket, "show-options", "-gv", "side-status"])
        .output()
        .unwrap()
        .status
        .success();
    if !supported {
        tmux(&["kill-server"]);
        fs::remove_dir_all(root).unwrap();
        assert_ne!(
            std::env::var_os("STARMUX_REQUIRE_SIDE_STATUS"),
            Some("1".into()),
            "tmux lacks side-status"
        );
        return;
    }
    for index in 1..28 {
        tmux(&[
            "new-window",
            "-d",
            "-t",
            "main:",
            "-n",
            &format!("row{index:02}"),
            "sleep 60",
        ]);
    }
    tmux(&[
        "set-environment",
        "-g",
        "STARMUX_CONFIG",
        config.to_str().unwrap(),
    ]);
    tmux(&[
        "set-environment",
        "-g",
        "XDG_CACHE_HOME",
        root.to_str().unwrap(),
    ]);
    tmux(&["set", "-g", "mouse", "on"]);
    tmux(&["set", "-g", "status-interval", "1"]);
    tmux(&["set", "-g", "side-status", "left"]);
    tmux(&["set", "-g", "side-status-width", "30"]);
    tmux(&["source-file", adapter_path.to_str().unwrap()]);
    let capture = root.join("client.out");
    let mut client = attached_client(&socket, "main")
        .env("TERM", "xterm-256color")
        .stdin(Stdio::piped())
        .stdout(Stdio::from(fs::File::create(&capture).unwrap()))
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut input = client.stdin.take().unwrap();
    let mut name = String::new();
    for _ in 0..40 {
        name = tmux(&["list-clients", "-F", "#{client_name}"]);
        if !name.is_empty() && fs::read_to_string(&capture).unwrap().contains("row01") {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    assert!(!name.is_empty());
    let socket_path = tmux(&["display-message", "-p", "-c", &name, "#{socket_path}"]);
    let focus = || tmux(&["display-message", "-p", "-c", &name, "#{window_id}"]);
    let initial_focus = focus();
    let render = || {
        let output = Command::new(binary)
            .args([
                "render-query",
                "--width=30",
                &format!("--socket={socket_path}"),
                &format!("--client={name}"),
            ])
            .env("STARMUX_CONFIG", &config)
            .env("XDG_CACHE_HOME", &root)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    };
    let first = render();
    assert!(
        first.contains("first") && !first.contains("row27"),
        "{first}"
    );
    thread::sleep(Duration::from_millis(350));
    let wheel = |button: u8, x: u8, y: u8, input: &mut std::process::ChildStdin| {
        write!(input, "\x1b[<{button};{x};{y}M").unwrap();
        input.flush().unwrap();
    };
    // Wheel over a clickable window row, then over a blank part of a row.
    wheel(65, 5, 3, &mut input);
    for _ in 0..30 {
        if !render().contains("#[range=user|st0 ") {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    assert!(
        !render().contains("#[range=user|st0 "),
        "wheel did not scroll"
    );
    assert_eq!(focus(), initial_focus);
    wheel(65, 25, 4, &mut input);
    for _ in 0..30 {
        if !render().contains("first") {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    assert!(!render().contains("first"), "blank space did not scroll");
    assert_eq!(focus(), initial_focus);

    let mut second_client = attached_client(&socket, "main")
        .env("TERM", "xterm-256color")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let _second_input = second_client.stdin.take();
    let mut other_name = String::new();
    for _ in 0..30 {
        other_name = tmux(&["list-clients", "-F", "#{client_name}"])
            .lines()
            .find(|candidate| *candidate != name)
            .unwrap_or_default()
            .to_owned();
        if !other_name.is_empty() {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    assert!(!other_name.is_empty());
    let other_render = Command::new(binary)
        .args([
            "render-query",
            "--width=30",
            &format!("--socket={socket_path}"),
            &format!("--client={other_name}"),
        ])
        .env("STARMUX_CONFIG", &config)
        .env("XDG_CACHE_HOME", &root)
        .output()
        .unwrap();
    assert!(other_render.status.success());
    assert!(String::from_utf8(other_render.stdout)
        .unwrap()
        .contains("#[range=user|st0 "));
    assert!(!render().contains("#[range=user|st0 "));

    wheel(64, 25, 4, &mut input);
    for _ in 0..30 {
        if render().contains("first") {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    assert!(render().contains("first"));
    assert_eq!(focus(), initial_focus);

    write!(input, "\x1b[<0;5;2M\x1b[<0;5;2m").unwrap();
    input.flush().unwrap();
    let second_window = tmux(&["display-message", "-p", "-t", "main:1", "#{window_id}"]);
    for _ in 0..30 {
        if focus() == second_window {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    assert_eq!(
        focus(),
        second_window,
        "click on scrolled row did not select it"
    );
    let height: u8 = tmux(&["display-message", "-p", "-c", &name, "#{client_height}"])
        .parse()
        .unwrap();
    wheel(64, 40, height, &mut input);
    for _ in 0..30 {
        if focus() == initial_focus {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    assert_eq!(
        focus(),
        initial_focus,
        "ordinary status wheel did not select a window"
    );

    for index in 0..32 {
        wheel(65, 25, 4, &mut input);
        thread::sleep(Duration::from_millis(35));
        if index == 20 {
            assert!(
                fs::read_to_string(&capture).unwrap().contains("row24"),
                "the sidebar waited until the gesture ended to paint"
            );
        }
    }
    for _ in 0..30 {
        if render().contains("row27") {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    assert!(render().contains("row27"), "last page was not reached");
    for _ in 0..30 {
        if fs::read_to_string(&capture).unwrap().contains("row27") {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    assert!(
        fs::read_to_string(&capture).unwrap().contains("row27"),
        "tmux did not paint the last window"
    );
    assert_eq!(focus(), initial_focus);
    for _ in 0..32 {
        wheel(64, 25, 4, &mut input);
        thread::sleep(Duration::from_millis(25));
    }
    for _ in 0..30 {
        if render().contains("#[range=user|st0 ") {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    assert!(
        render().contains("#[range=user|st0 "),
        "first page was not reached"
    );
    for _ in 0..12 {
        wheel(65, 25, 4, &mut input);
        wheel(64, 25, 4, &mut input);
    }
    thread::sleep(Duration::from_millis(800));
    assert!(
        render().contains("#[range=user|st0 "),
        "rapid reversal left an offset"
    );
    assert_eq!(focus(), initial_focus);
    assert!(!fs::read_to_string(&capture)
        .unwrap()
        .contains("starmux: input error"));

    tmux(&["kill-server"]);
    let _ = second_client.wait();
    let _ = client.wait();
    fs::remove_dir_all(root).unwrap();
}
