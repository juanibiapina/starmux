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
    assert!(String::from_utf8_lossy(&valid.stdout).contains("#[range=window|"));
    let mut stale = query.clone();
    stale[5] = "--current-window=@999999999".into();
    let rejected = Command::new(binary)
        .args(&stale)
        .env("STARMUX_CONFIG", &config_path)
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("tmux focus changed during query"));

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
        assert!(padded.contains("----------------------------#[nl]"));
    }
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
            "version": 1, "sessionId": id,
            "plans": [{"id": "0123456789abcdef01234567", "title": format!("{id} plan"), "path": plan_relative}],
            "pullRequests": [], "skills": ["testing"], "skillPaths": {"testing": skill_path}
        }).to_string()).unwrap();
        let status = serde_json::json!({
            "version": 1, "sessionId": id, "name": name, "pid": 1,
            "cwd": "/tmp", "socketPath": pi_socket, "sessionFile": session_file,
            "contextPath": context_path, "startedAt": "2026-01-01T00:00:00Z",
            "updatedAt": "2026-01-01T00:00:00Z", "state": state,
            "tmux": {"paneId": pane, "sessionName": "main", "windowIndex": 0, "windowName": "old", "socketPath": server_socket}
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
            "version": 1, "sessionId": "foreign", "name": "foreign pi", "pid": 1,
            "cwd": "/tmp", "socketPath": pi_socket,
            "startedAt": "2026-01-01T00:00:00Z", "updatedAt": "2026-01-01T00:00:00Z",
            "state": "idle",
            "tmux": {"paneId": foreign_pane, "sessionName": "main", "socketPath": foreign_server_socket}
        }).to_string(),
    ).unwrap();
    let config = root.join("config.toml");
    fs::write(
        &config,
        format!(
            "modules = [\"sessions\", \"divider\", \"pi-live\", \"divider\", \"pi-context\"]\n[pi-live]\ndata_dir = {:?}\n[pi-context]\nopen_command = [\"dev\", \"tmux\", \"edit\", \"{{file}}\", \"{{pane}}\", \"{{socket}}\"]\n",
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
