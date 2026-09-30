use starmux::usage::{RefreshFailure, UsageRow, UsageWindow};
use starmux::{
    Application, Diagnostics, GitStatus, GobJob, Pane, PiContext, PiPlan, PiSession, PiSkill,
    PiTarget, ProcessTmux, RenderInputs, Session, Sidebar, Snapshot, Tmux, Window,
};
use std::{collections::BTreeMap, path::Path, sync::Mutex};

fn snapshot() -> Snapshot {
    Snapshot {
        width: 40,
        client_width: 100,
        client_height: 60,
        status_lines: 1,
        current_session: "$0".into(),
        current_pane: "%0".into(),
        pane_path: "/tmp".into(),
        sessions: vec![Session {
            id: "$0".into(),
            name: "main".into(),
            windows: vec![Window {
                id: "@0".into(),
                index: 0,
                name: "code".into(),
                selected: true,
                pane: "%0".into(),
                path: "/tmp".into(),
                options: BTreeMap::new(),
            }],
        }],
    }
}
fn tokens(text: &str) -> Vec<String> {
    text.split("#[range=user|")
        .skip(1)
        .map(|part| part.split(' ').next().unwrap().to_owned())
        .collect()
}
struct Fixture {
    jobs: Vec<GobJob>,
    usage: Vec<UsageRow>,
    pi: Vec<PiSession>,
    context: PiContext,
    commands: BTreeMap<String, String>,
    git: GitStatus,
    debug: Diagnostics,
}
impl Fixture {
    fn new() -> Self {
        Self {
            jobs: vec![GobJob {
                id: "build-1".into(),
                name: "Build assets".into(),
                started_at: Some(time::OffsetDateTime::now_utc()),
                avg_duration_ms: 1000,
            }],
            usage: vec![UsageRow {
                provider: "anthropic".into(),
                display_name: "Claude".into(),
                windows: vec![UsageWindow {
                    label: "Five hour".into(),
                    duration_seconds: Some(18000),
                    used_percent: 20.0,
                    reset_at: None,
                }],
                available_resets: None,
                stale: false,
                fetched_at: Some(1),
                unavailable: false,
                refresh_failure: Some(RefreshFailure::RefreshFailed),
            }],
            pi: vec![PiSession {
                name: "Agent".into(),
                project: "/tmp".into(),
                state: "idle".into(),
                location: None,
                target: Some(PiTarget {
                    pane: "%0".into(),
                    window: "@0".into(),
                }),
                selected: true,
            }],
            context: PiContext {
                plans: vec![PiPlan {
                    title: "Plan".into(),
                    path: "/tmp/plan.md".into(),
                }],
                skills: vec![PiSkill {
                    name: "documentation".into(),
                    path: None,
                }],
                pull_requests: vec!["https://github.com/owner/repo/pull/1".into()],
            },
            commands: BTreeMap::from([("command.test".into(), "healthy".into())]),
            git: GitStatus {
                branch: "main".into(),
                ..GitStatus::default()
            },
            debug: Diagnostics { stages: [10; 9] },
        }
    }
    fn input(&self) -> RenderInputs<'_> {
        RenderInputs {
            top: None,
            gob_jobs: &self.jobs,
            usage_rows: &self.usage,
            pi_sessions: &self.pi,
            context: Some(&self.context),
            commands: &self.commands,
            git: Some(&self.git),
            debug: Some(&self.debug),
            states: &[],
        }
    }
}
const MODULE_CONFIG: &str = r#"
modules = ["top", "sessions", "pi-workbench", "usage", "gob", "pi-context", "git", "command.test", "debug", "divider", "blank", "divider", "blank", "spacer"]
[commands.test]
argv = ["echo", "healthy"]
[debug]
details = true
[usage]
providers = ["anthropic"]
[actions.all]
argv = ["echo", "{module}", "{kind}", "{part}"]
[[clicks]]
action = "all"
"#;

#[test]
fn global_action_covers_every_module_row_including_headings_and_padding() {
    let sidebar = Sidebar::from_toml(MODULE_CONFIG).unwrap();
    let fixture = Fixture::new();
    let rendered = sidebar
        .render_with_inputs(&snapshot(), fixture.input())
        .unwrap();
    let targets = tokens(&rendered);
    assert_eq!(targets.len(), 59);
    assert!(
        targets
            .iter()
            .all(|target| target.starts_with("sc") && target.len() == 15),
        "{targets:?}"
    );
    assert_eq!(
        targets
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        targets.len()
    );
    let roundtrip = Sidebar::from_toml(&sidebar.print_config().unwrap()).unwrap();
    assert_eq!(
        tokens(
            &roundtrip
                .render_with_inputs(&snapshot(), fixture.input())
                .unwrap()
        ),
        targets
    );
    let scrolled = sidebar
        .render_scrolled(&snapshot(), fixture.input(), 10)
        .unwrap();
    assert_eq!(tokens(&scrolled.0), targets); // Content fits; offset clamps to zero.
}

#[test]
fn rules_restore_defaults_disable_rows_and_select_individual_items() {
    let config = format!("{MODULE_CONFIG}\n[[clicks]]\nmodule = \"sessions\"\naction = \"default\"\n[[clicks]]\nmodule = \"top\"\nmatch = {{ metric = \"memory\" }}\naction = \"none\"\n[[clicks]]\nmodule = \"gob\"\nmatch = {{ job_id = \"build-1\", part = \"progress\" }}\naction = \"none\"\n[[clicks]]\nmodule = \"blank\"\ninstance = 2\naction = \"none\"\n");
    let rendered = Sidebar::from_toml(&config)
        .unwrap()
        .render_with_inputs(&snapshot(), Fixture::new().input())
        .unwrap();
    let targets = tokens(&rendered);
    assert_eq!(
        targets
            .iter()
            .filter(|target| target.as_str() == "sv")
            .count(),
        3
    );
    assert!(targets.contains(&"st0".into()));
    assert!(targets.contains(&"sw0".into()));
}

#[test]
fn job_identity_survives_sorting_and_scrolling_and_usage_can_match_duration() {
    let sidebar = Sidebar::from_toml(
        r#"
modules = ["gob", "usage"]
[usage]
providers = ["anthropic"]
[actions.job]
argv = ["echo", "{job_id}"]
[actions.usage]
argv = ["echo", "{provider}", "{label}"]
[[clicks]]
module = "gob"
match = { kind = "job", job_id = "build-1" }
action = "job"
[[clicks]]
module = "usage"
match = { kind = "window", provider = "anthropic", duration_seconds = 18000 }
action = "usage"
"#,
    )
    .unwrap();
    let mut fixture = Fixture::new();
    let before = tokens(
        &sidebar
            .render_with_inputs(&snapshot(), fixture.input())
            .unwrap(),
    );
    assert_eq!(
        before
            .iter()
            .filter(|token| token.starts_with("scf"))
            .count(),
        2
    );
    assert_eq!(
        before
            .iter()
            .filter(|token| token.starts_with("sce"))
            .count(),
        1
    );
    fixture.jobs.insert(
        0,
        GobJob {
            id: "other".into(),
            name: "AAA".into(),
            started_at: None,
            avg_duration_ms: 0,
        },
    );
    let after = tokens(
        &sidebar
            .render_with_inputs(&snapshot(), fixture.input())
            .unwrap(),
    );
    assert_eq!(
        before
            .iter()
            .filter(|t| t.starts_with("sc"))
            .collect::<Vec<_>>(),
        after
            .iter()
            .filter(|t| t.starts_with("sc"))
            .collect::<Vec<_>>()
    );
    let mut short = snapshot();
    short.client_height = 4;
    let page = sidebar.render_scrolled(&short, fixture.input(), 2).unwrap();
    assert!(tokens(&page.0).iter().any(|t| t.starts_with("scf")));
}

#[test]
fn selectors_and_placeholders_are_validated_before_any_source_query() {
    for tail in [
        "[[clicks]]\nmodule = 'missing'\naction = 'none'",
        "[[clicks]]\nmodule = 'top'\nmatch = {job_id = 'x'}\naction = 'none'",
        "[[clicks]]\nmodule = 'top'\nmatch = {kind = 'missing'}\naction = 'none'",
        "[[clicks]]\nmodule = 'top'\naction = 'missing'",
        "[actions.none]\nargv = ['echo']",
        "[actions.x]\nargv = []",
        "[actions.x]\nargv = ['{socket}']",
        "[actions.x]\nargv = ['echo', '{file}']\n[[clicks]]\nmodule = 'top'\naction = 'x'",
        "[actions.x]\nargv = ['echo', '{unknown}']\n[[clicks]]\naction = 'x'",
        "[[clicks]]\ninstance = 0\naction = 'none'",
        "[[clicks]]\nconfig = 'missing'\naction = 'none'",
        "[[clicks]]\nmodule = 'usage'\nmatch = {duration_seconds = '18000'}\naction = 'none'",
    ] {
        assert!(
            Sidebar::from_toml(&format!("modules = ['top']\n{tail}")).is_err(),
            "accepted {tail}"
        );
    }
}

struct LocalTmux {
    state: Mutex<Snapshot>,
}
impl Tmux for LocalTmux {
    fn panes(&self, _: &str) -> Result<Vec<Pane>, String> {
        Ok(vec![])
    }
    fn snapshot(
        &self,
        _: &str,
        _: &str,
        width: usize,
        _: &[(String, String)],
    ) -> Result<Snapshot, String> {
        let mut snapshot = self.state.lock().unwrap().clone();
        snapshot.width = width;
        Ok(snapshot)
    }
    fn activate(&self, _: &str, _: &str, _: &str) -> Result<(), String> {
        Err("unexpected default activation".into())
    }
    fn open_url(&self, _: &str) -> Result<(), String> {
        Err("unexpected browser".into())
    }
    fn open_file(&self, _: &Path, _: Option<&[std::ffi::OsString]>) -> Result<(), String> {
        Err("unexpected opener".into())
    }
    fn run_action(&self, args: &[String], directory: &Path) -> Result<(), String> {
        ProcessTmux.run_action(args, directory)
    }
}

#[test]
fn activation_preserves_literal_arguments_and_rejects_changed_focus_and_definitions() {
    use std::fs;
    let root = std::env::temp_dir().join(format!("starmux-action-args-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let root = fs::canonicalize(root).unwrap();
    let script = root.join("record.sh");
    let output = root.join("args");
    fs::write(&script, "printf '%s\\n' \"$PWD\" \"$@\" > \"$1\"\n").unwrap();
    let config = format!(
        r#"
modules = ["sessions"]
[actions.record]
argv = ["/bin/sh", {:?}, {:?}, "{{session}}:4", "{{socket}}", "{{client}}", "{{name}}", "{{{{literal}}}}"]
[[clicks]]
module = "sessions"
match = {{ kind = "window" }}
action = "record"
"#,
        script.display().to_string(),
        output.display().to_string()
    );
    let sidebar = Sidebar::from_toml(&config).unwrap();
    let mut state = snapshot();
    state.pane_path = root.display().to_string();
    state.sessions[0].windows[0].name = "quotes ' $() #{pane_id} {socket}".into();
    let target = tokens(&sidebar.render(&state).unwrap())
        .into_iter()
        .find(|t| t.starts_with("sc"))
        .unwrap();
    let tmux = LocalTmux {
        state: Mutex::new(state.clone()),
    };
    let app = Application::new(sidebar, &tmux);
    app.activate("socket with spaces", "client ' literal", &target)
        .unwrap();
    let contents = fs::read_to_string(&output).unwrap();
    assert_eq!(
        contents.lines().collect::<Vec<_>>(),
        vec![
            root.to_str().unwrap(),
            output.to_str().unwrap(),
            "$0:4",
            "socket with spaces",
            "client ' literal",
            "quotes ' $() #{pane_id} {socket}",
            "{literal}"
        ]
    );
    fs::remove_file(&output).unwrap();
    tmux.state.lock().unwrap().current_pane = "%1".into();
    assert!(app.activate("socket", "client", &target).is_err());
    assert!(!output.exists());
    *tmux.state.lock().unwrap() = state;
    let changed = Application::new(
        Sidebar::from_toml(&config.replace("{session}:4", "{session}:5")).unwrap(),
        &tmux,
    );
    assert!(changed.activate("socket", "client", &target).is_err());
    assert!(!output.exists());
    assert!(app.activate("socket", "client", "scaINVALID").is_err());
    fs::remove_dir_all(root).unwrap();
}
impl Tmux for &LocalTmux {
    fn panes(&self, socket: &str) -> Result<Vec<Pane>, String> {
        (*self).panes(socket)
    }
    fn snapshot(
        &self,
        socket: &str,
        client: &str,
        width: usize,
        options: &[(String, String)],
    ) -> Result<Snapshot, String> {
        (*self).snapshot(socket, client, width, options)
    }
    fn activate(&self, socket: &str, client: &str, token: &str) -> Result<(), String> {
        (*self).activate(socket, client, token)
    }
    fn open_url(&self, url: &str) -> Result<(), String> {
        (*self).open_url(url)
    }
    fn open_file(&self, file: &Path, command: Option<&[std::ffi::OsString]>) -> Result<(), String> {
        (*self).open_file(file, command)
    }
    fn run_action(&self, args: &[String], directory: &Path) -> Result<(), String> {
        (*self).run_action(args, directory)
    }
}

#[test]
fn execution_reports_nonzero_exit_missing_program_output_limit_and_timeout() {
    let tmux = ProcessTmux;
    for (args, expected) in [
        (
            vec!["/bin/sh", "-c", "printf failure >&2; exit 7"],
            "failure",
        ),
        (vec!["/nonexistent/starmux-action"], "start click action"),
        (vec!["/bin/sh", "-c", "exec sleep 10"], "timed out"),
        (
            vec![
                "/bin/sh",
                "-c",
                "dd if=/dev/zero bs=20000 count=1 >&2 2>/dev/null",
            ],
            "exceeds 16 KiB",
        ),
    ] {
        let error = tmux
            .run_action(
                &args.into_iter().map(str::to_owned).collect::<Vec<_>>(),
                Path::new("/tmp"),
            )
            .unwrap_err();
        assert!(error.contains(expected), "{error}");
    }
}
