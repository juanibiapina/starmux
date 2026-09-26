#[cfg(target_os = "macos")]
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
        assert!(
            std::env::var_os("STARMUX_REQUIRE_SIDE_STATUS").is_none(),
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
    let mut client = Command::new("script")
        .args([
            "-q",
            "/dev/null",
            "tmux",
            "-L",
            &socket,
            "attach",
            "-t",
            "main",
        ])
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
    assert!(String::from_utf8_lossy(&valid.stdout).contains("#[range=window|"));
    let mut stale = query;
    stale[5] = "--current-window=@999999999".into();
    let rejected = Command::new(binary)
        .args(&stale)
        .env("STARMUX_CONFIG", &config_path)
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("tmux focus changed during query"));
    tmux(&["kill-server"]);
    let _ = client.wait();
    fs::remove_dir_all(root).unwrap();
}

#[cfg(target_os = "macos")]
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
        assert!(
            std::env::var_os("STARMUX_REQUIRE_SIDE_STATUS").is_none(),
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
    let mut client = Command::new("script")
        .args([
            "-q",
            "/dev/null",
            "tmux",
            "-L",
            &socket,
            "attach",
            "-t",
            "main",
        ])
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
