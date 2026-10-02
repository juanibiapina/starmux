use starmux::{PiSession, PiTarget, Session, Sidebar, Snapshot, Window};
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::PathBuf,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use vt100::{Color, Parser, Screen};

struct Client {
    root: PathBuf,
    socket: String,
    child: Option<Child>,
}

impl Client {
    fn start() -> Option<Self> {
        let socket = format!("starmux-background-{}", std::process::id());
        let root = std::env::temp_dir().join(&socket);
        fs::create_dir_all(&root).unwrap();
        let mut client = Self {
            root,
            socket,
            child: None,
        };
        client.tmux(&[
            "-f",
            "/dev/null",
            "new-session",
            "-d",
            "-s",
            "main",
            "sleep 60",
        ]);
        let supported = Command::new("tmux")
            .args(["-L", &client.socket, "show-options", "-gv", "side-status"])
            .output()
            .unwrap()
            .status
            .success();
        if !supported {
            assert_ne!(
                std::env::var_os("STARMUX_REQUIRE_SIDE_STATUS"),
                Some("1".into()),
                "tmux lacks side-status"
            );
            return None;
        }
        client.tmux(&["set", "-g", "status", "off"]);
        client.tmux(&["set", "-g", "mouse", "on"]);
        client.tmux(&["set", "-g", "side-status", "left"]);
        client.tmux(&["set", "-g", "side-status-width", "31"]);
        client.tmux(&["set", "-g", "side-status-style", "fg=colour7,bg=colour0"]);
        client.child = Some(
            super::attached_client(&client.socket, "main")
                .env("TERM", "xterm-256color")
                .stdin(Stdio::piped())
                .stdout(Stdio::from(
                    fs::File::create(client.root.join("client.out")).unwrap(),
                ))
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        );
        Some(client)
    }

    fn tmux(&self, args: &[&str]) -> String {
        let output = Command::new("tmux")
            .args(["-L", &self.socket])
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "tmux {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    }

    fn screen(&self) -> Screen {
        let size = self.tmux(&["list-clients", "-F", "#{client_height} #{client_width}"]);
        let mut size = size
            .split_whitespace()
            .map(|value| value.parse::<u16>().unwrap());
        let mut parser = Parser::new(size.next().unwrap_or(24), size.next().unwrap_or(80), 0);
        parser.process(&fs::read(self.root.join("client.out")).unwrap());
        parser.screen().clone()
    }

    fn paint(
        &self,
        sidebar: &Sidebar,
        sessions: &[PiSession],
        marker: &str,
        width: usize,
    ) -> Screen {
        let snapshot = Snapshot {
            width,
            client_width: 80,
            client_height: 24,
            status_lines: 0,
            current_session: "$0".into(),
            current_pane: "%0".into(),
            pane_path: "/tmp".into(),
            sessions: vec![Session {
                id: "$0".into(),
                name: "main".into(),
                windows: vec![Window {
                    id: "@0".into(),
                    index: 0,
                    name: "main".into(),
                    selected: true,
                    pane: "%0".into(),
                    path: "/tmp".into(),
                    options: BTreeMap::new(),
                }],
            }],
        };
        let format = sidebar
            .render_with_pi_workbench(&snapshot, sessions)
            .unwrap();
        self.tmux(&["set", "-g", "side-status-width", &(width + 1).to_string()]);
        self.tmux(&["set", "-g", "side-status-format", &format]);
        self.tmux(&["refresh-client", "-S"]);
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let screen = self.screen();
            if screen.contents().contains(marker) {
                return screen;
            }
            assert!(
                Instant::now() < deadline,
                "missing {marker:?}: {}",
                screen.contents()
            );
            thread::sleep(Duration::from_millis(50));
        }
    }

    fn click_last_column(&mut self, row: u16, expected: &str) {
        self.tmux(&[
            "bind",
            "-n",
            "MouseDown1Status",
            "set-option",
            "-gF",
            "@background-click",
            "#{mouse_status_range}",
        ]);
        let input = self.child.as_mut().unwrap().stdin.as_mut().unwrap();
        write!(input, "\x1b[<0;30;{}M\x1b[<0;30;{}m", row + 1, row + 1).unwrap();
        input.flush().unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if self.tmux(&["show", "-gqv", "@background-click"]) == expected {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "last content column did not retain range {expected}"
            );
            thread::sleep(Duration::from_millis(50));
        }
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        let _ = Command::new("tmux")
            .args(["-L", &self.socket, "kill-server"])
            .output();
        if let Some(child) = &mut self.child {
            let _ = child.wait();
        }
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn session(name: &str, project: &str, selected: bool) -> PiSession {
    PiSession {
        name: name.into(),
        project: project.into(),
        state: "idle".into(),
        location: None,
        target: Some(PiTarget {
            pane: "%0".into(),
            window: "@0".into(),
        }),
        selected,
    }
}

fn row_named(screen: &Screen, name: &str) -> u16 {
    screen
        .rows(0, 30)
        .position(|row| row.contains(name))
        .unwrap_or_else(|| panic!("missing {name}: {}", screen.contents())) as u16
}

fn assert_background(screen: &Screen, row: u16, width: u16, color: Color) {
    for column in 0..width {
        assert_eq!(
            screen.cell(row, column).unwrap().bgcolor(),
            color,
            "row {row}, column {column}: {}",
            screen.contents()
        );
    }
    assert_eq!(
        screen.cell(row, width).unwrap().contents(),
        "│",
        "sidebar border"
    );
}

#[test]
fn selected_background_stays_within_its_row() {
    let Some(mut client) = Client::start() else {
        return;
    };
    let sidebar = Sidebar::from_toml(
        r#"
modules = ["pi-workbench"]
[pi-workbench]
selected_style = "fg=colour7,bg=colour1,bold"
selected_fill = "colour1"
"#,
    )
    .unwrap();
    let screen = client.paint(
        &sidebar,
        &[
            session("selected", "/projects/starmux", true),
            session("following", "/projects/deltoids", false),
        ],
        "following",
        30,
    );
    let selected = row_named(&screen, "selected");
    assert_background(&screen, selected, 30, Color::Idx(1));
    assert_background(&screen, row_named(&screen, "deltoids"), 30, Color::Idx(0));
    assert_background(&screen, row_named(&screen, "following"), 30, Color::Idx(0));
    client.click_last_column(selected, "sp0");

    let screen = client.paint(
        &sidebar,
        &[
            session("previous", "/projects/starmux", false),
            session("new selection", "/projects/starmux", true),
            session("after movement", "/projects/deltoids", false),
        ],
        "after movement",
        30,
    );
    assert_background(&screen, row_named(&screen, "previous"), 30, Color::Idx(0));
    assert_background(
        &screen,
        row_named(&screen, "new selection"),
        30,
        Color::Idx(1),
    );
    assert_background(&screen, row_named(&screen, "deltoids"), 30, Color::Idx(0));

    let screen = client.paint(
        &sidebar,
        &[session("last selected", "/projects/starmux", true)],
        "last selected",
        30,
    );
    let last = row_named(&screen, "last selected");
    assert_background(&screen, last, 30, Color::Idx(1));
    assert_background(&screen, last + 1, 30, Color::Idx(0));

    let screen = client.paint(
        &sidebar,
        &[
            session("slim selected", "/projects/starmux", true),
            session("slim following", "/projects/deltoids", false),
        ],
        "○○",
        2,
    );
    assert_eq!(screen.cell(0, 0).unwrap().contents(), "○");
    assert_eq!(screen.cell(0, 1).unwrap().contents(), "●");
    assert_background(&screen, 0, 2, Color::Idx(1));
    assert_background(&screen, 1, 2, Color::Idx(0));
    assert_background(&screen, 2, 2, Color::Idx(0));
}
