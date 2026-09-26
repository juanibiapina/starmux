use starmux::{PiSession, PiTarget, Session, Sidebar, Snapshot, Window};
use std::collections::BTreeMap;

fn snapshot() -> Snapshot {
    Snapshot {
        width: 30,
        client_width: 100,
        client_height: 25,
        current_session: "$0".into(),
        current_pane: "%0".into(),
        pane_path: "/tmp".into(),
        sessions: vec![
            Session {
                id: "$0".into(),
                name: "main".into(),
                windows: vec![
                    Window {
                        id: "@0".into(),
                        index: 0,
                        name: "code".into(),
                        selected: true,
                        pane: "%0".into(),
                        path: "/tmp".into(),
                        options: BTreeMap::new(),
                    },
                    Window {
                        id: "@1".into(),
                        index: 1,
                        name: "#[fg=red] x#{oops}".into(),
                        selected: false,
                        pane: "%1".into(),
                        path: "/tmp".into(),
                        options: BTreeMap::from([
                            ("icon".into(), "π".into()),
                            ("state".into(), "working".into()),
                        ]),
                    },
                ],
            },
            Session {
                id: "$1".into(),
                name: "other".into(),
                windows: vec![Window {
                    id: "@2".into(),
                    index: 0,
                    name: "notes".into(),
                    selected: true,
                    pane: "%2".into(),
                    path: "/tmp".into(),
                    options: BTreeMap::from([
                        ("icon".into(), String::new()),
                        ("state".into(), "notify".into()),
                    ]),
                }],
            },
        ],
    }
}

#[test]
fn portable_defaults_render_navigation_without_personal_options() {
    let sidebar = Sidebar::defaults().unwrap();
    assert!(sidebar.requested_window_options().is_empty());
    let rendered = sidebar.render(&snapshot()).unwrap();
    assert!(rendered.contains("#[range=session|$0 "));
    assert!(rendered.contains("#[range=window|0 list=focus "));
    assert!(rendered.contains("#[range=user|sw1z141z6 "));
    assert!(rendered.contains("##[fg=red] x##{oops}"));
    assert!(rendered.contains("----------------------------"));
    assert!(!rendered.contains("●"));
    assert!(!rendered.contains("#c099ff"));
}

#[test]
fn configured_formats_palettes_and_indicator_rules_recreate_personal_presentation() {
    let config = r##"
modules = ["sessions", "divider"]
palette = "tokyo"

[palettes.tokyo]
background = "#1b1d2b"
surface = "#1e2030"
highlight = "#292e42"
border = "#3b4261"
text = "#82aaff"
muted = "#828bb8"
accent = "#c099ff"
warning = "#ffc777"

[sessions]
session_format = "[ $name ]($style)"
window_format = "[ $index ]($style)$indicator[ $name]($style)"
current_session_style = "fg=background,bg=accent,bold"
other_session_style = "fg=text,bg=highlight,bold"
active_window_style = "fg=accent,bg=border,bold"
selected_window_style = "fg=muted,bg=surface,bold"
other_window_style = "fg=muted,bg=surface"
current_session_fill = "accent"
other_session_fill = "highlight"
active_window_fill = "border"
selected_window_fill = "background"
other_window_fill = "background"

[sessions.window_options]
icon = "@window_icon"
state = "@pi_win_state"

[sessions.indicator]
source = "icon"
fallback = "|"
style = "$style"

[[sessions.indicator.rules]]
when = { state = "working" }
text = "●"
style = "fg=warning"

[[sessions.indicator.rules]]
when = { state = "notify" }
text = "●"
style = "fg=accent"

[divider]
character = "─"
style = "fg=border,nobold"
"##;
    let sidebar = Sidebar::from_toml(config).unwrap();
    assert_eq!(
        sidebar.requested_window_options(),
        vec![
            ("icon".into(), "@window_icon".into()),
            ("state".into(), "@pi_win_state".into())
        ]
    );
    let rendered = sidebar.render(&snapshot()).unwrap();
    assert!(rendered.contains("#[fg=#ffc777]●"), "{rendered}");
    assert!(rendered.contains("#[fg=#c099ff]●"), "{rendered}");
    assert!(rendered.contains("#[fg=#3b4261,nobold] ─"), "{rendered}");
    assert!(rendered.contains("#[fill=#c099ff]"), "{rendered}");
}

#[test]
fn formatter_rejects_unknown_or_unsafe_configuration() {
    for config in [
        "modules = [\"missing\"]",
        "palette = \"missing\"",
        "[sessions]\ncurrent_session_style = \"fg=not-a-color\"",
        "[sessions]\nwindow_format = \"$unknown\"",
        "[sessions]\nwindow_format = \"$style\"",
        "[sessions]\nwindow_format = \"[x](fg=red]#[range=user|bad)\"",
        "[sessions.window_options]\nicon = \"window_name\"",
        "[divider]\ncharacter = \"--\"",
    ] {
        assert!(Sidebar::from_toml(config).is_err(), "accepted {config}");
    }
}

#[test]
fn module_order_disabling_optional_groups_and_clipping_are_observable_at_the_interface() {
    let sidebar = Sidebar::from_toml(
        r#"
modules = ["divider", "sessions"]

[sessions]
window_format = "[$index: $name](fg=red)( $indicator)"

[divider]
character = "="
"#,
    )
    .unwrap();
    let mut input = snapshot();
    input.width = 12;
    input.sessions[0].windows[0].name = "界界界界界界".into();
    let rendered = sidebar.render(&input).unwrap();
    let divider = rendered.find("==========").unwrap();
    let session = rendered.find("#[range=session|$0 ").unwrap();
    assert!(divider < session, "module order was not preserved");
    assert!(!rendered.contains("indicator"));
    assert!(!rendered.contains("界界界界界界"));

    let disabled = Sidebar::from_toml("modules = [\"divider\"]\n[divider]\ndisabled = true")
        .unwrap()
        .render(&input)
        .unwrap();
    assert_eq!(disabled.matches("#[nl]").count(), 2);
}

#[test]
fn pi_live_rows_follow_module_order_and_escape_session_names() {
    let sidebar = Sidebar::from_toml(
        r##"
modules = ["pi-live", "divider"]
palette = "tokyo"
[palettes.tokyo]
warning = "#ffc777"
muted = "#828bb8"
[pi-live]
format = " $state $name"
working_style = "fg=warning"
idle_style = "fg=muted"
"##,
    )
    .unwrap();
    let mut input = snapshot();
    input.width = 50;
    let rendered = sidebar
        .render_with_pi_live(
            &input,
            &[
                PiSession {
                    state: "working".into(),
                    project: "/projects/starmux".into(),
                    name: "#[range=user|bad]#{oops}".into(),
                    location: None,
                    target: None,
                    selected: false,
                },
                PiSession {
                    state: "idle".into(),
                    project: "/projects/starmux".into(),
                    name: "waiting".into(),
                    location: None,
                    target: None,
                    selected: false,
                },
            ],
        )
        .unwrap();
    assert!(
        rendered.contains("#[fg=#ffc777]●#[default] ##[range=user|bad]##{oops}"),
        "{rendered}"
    );
    assert!(
        rendered.contains("#[fg=#828bb8]●#[default] waiting"),
        "{rendered}"
    );
    assert!(rendered.find("waiting").unwrap() < rendered.find("----------").unwrap());
    assert!(!rendered.contains("#[range=user|bad]#{oops}"));
    assert!(!rendered.contains("#[range=window|"));

    for config in [
        "modules = [\"pi-live\"]\n[pi-live]\nformat = \"$cwd\"",
        "modules = [\"pi-live\"]\n[pi-live]\nworking_style = \"fg=#[bad]\"",
        "modules = [\"pi-live\"]\n[pi-live]\ndata_dir = \"relative\"",
    ] {
        assert!(Sidebar::from_toml(config).is_err(), "accepted {config}");
    }
}

#[test]
fn pi_projects_stay_together_in_priority_order() {
    let sidebar = Sidebar::from_toml("modules = [\"pi-live\"]").unwrap();
    let session = |project: &str, name: &str, state: &str, selected: bool| PiSession {
        name: name.into(),
        project: format!("/projects/{project}"),
        state: state.into(),
        location: None,
        target: selected.then(|| PiTarget {
            pane: "%1".into(),
            window: "@2".into(),
        }),
        selected,
    };
    let rendered = sidebar
        .render_with_pi_live(
            &snapshot(),
            &[
                session("idle", "only idle", "idle", false),
                session("working", "doing work", "working", false),
                session("alert", "later idle", "idle", false),
                session("selected", "current idle", "idle", true),
                session("alert", "needs attention", "notify", false),
            ],
        )
        .unwrap();
    let positions: Vec<_> = [
        "#[bold] alert",
        "needs attention",
        "later idle",
        "#[bold] working",
        "doing work",
        "#[bold] selected",
        "current idle",
        "#[bold] idle",
        "only idle",
    ]
    .iter()
    .map(|text| rendered.find(text).expect(text))
    .collect();
    assert!(
        positions.windows(2).all(|pair| pair[0] < pair[1]),
        "{rendered}"
    );
    assert!(rendered.contains("#[fg=magenta]●"), "{rendered}");
    assert!(rendered.contains("#[range=user|sp"), "{rendered}");

    let duplicates = sidebar
        .render_with_pi_live(
            &snapshot(),
            &[
                session("owner-a/app", "alpha", "idle", false),
                session("owner-b/app", "beta", "idle", false),
            ],
        )
        .unwrap();
    assert!(duplicates.contains("owner-a/app"), "{duplicates}");
    assert!(duplicates.contains("owner-b/app"), "{duplicates}");
}

#[test]
fn selected_pi_background_does_not_fill_following_read_only_row() {
    let sidebar = Sidebar::from_toml(
        r##"
modules = ["pi-live"]
palette = "tokyo"
[palettes.tokyo]
accent = "#c099ff"
border = "#3b4261"
[pi-live]
selected_style = "fg=accent,bg=border,bold"
selected_fill = "border"
"##,
    )
    .unwrap();
    let rows = [
        PiSession {
            name: "selected".into(),
            project: "/projects/starmux".into(),
            state: "working".into(),
            location: None,
            target: Some(PiTarget {
                pane: "%1".into(),
                window: "@2".into(),
            }),
            selected: true,
        },
        PiSession {
            name: "read only".into(),
            project: "/projects/starmux".into(),
            state: "idle".into(),
            location: None,
            target: None,
            selected: false,
        },
    ];
    let rendered = sidebar.render_with_pi_live(&snapshot(), &rows).unwrap();
    let lines: Vec<_> = rendered.split("#[nl]").collect();
    let selected = lines.iter().find(|line| line.contains("selected")).unwrap();
    let read_only = lines
        .iter()
        .find(|line| line.contains("read only"))
        .unwrap();
    assert!(selected.contains("#[fill=#3b4261]"), "{rendered}");
    assert!(read_only.contains("read only#[bg=default]"), "{rendered}");
    assert!(
        read_only.contains("          #[fill=default]"),
        "{rendered}"
    );
}

#[test]
fn configured_option_values_remain_literal_data() {
    let sidebar = Sidebar::from_toml(
        r#"
modules = ["sessions"]
[sessions]
window_format = "$indicator$name"
[sessions.window_options]
icon = "@icon"
[sessions.indicator]
source = "icon"
"#,
    )
    .unwrap();
    let mut input = snapshot();
    input.sessions[0].windows[0]
        .options
        .insert("icon".into(), "#[range=user|bad]#{pane_id}".into());
    let rendered = sidebar.render(&input).unwrap();
    assert!(rendered.contains("##[range=user|bad]##{pane_id}"));
    assert!(!rendered.contains("#[range=user|bad]#{pane_id}"));
}
