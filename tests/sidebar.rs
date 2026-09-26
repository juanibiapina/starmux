use starmux::usage::{UsageRow, UsageWindow};
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
fn divider_can_separate_usage_from_pi_sessions() {
    let sidebar = Sidebar::from_toml(
        r#"
modules = ["sessions", "divider", "pi-live", "divider", "usage"]
[divider]
character = "─"
[usage]
providers = ["codex"]
"#,
    )
    .unwrap();
    let usage = UsageRow {
        provider: "codex".into(),
        display_name: "Codex Plan".into(),
        windows: vec![UsageWindow {
            label: "5h".into(),
            used_percent: 40.0,
            duration_seconds: None,
            reset_at: None,
        }],
        stale: false,
        fetched_at: Some(100),
        unavailable: false,
    };
    let pi = PiSession {
        state: "idle".into(),
        project: "/projects/starmux".into(),
        name: "pi session".into(),
        location: None,
        target: None,
        selected: false,
    };
    let rendered = sidebar
        .render_with_usage(&snapshot(), &[pi], &[usage])
        .unwrap();
    let lines: Vec<_> = rendered.split("#[nl]").collect();
    let dividers: Vec<_> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.contains("─"))
        .map(|(index, _)| index)
        .collect();
    let pi = lines
        .iter()
        .position(|line| line.contains("pi session"))
        .unwrap();
    let codex = lines
        .iter()
        .position(|line| line.contains("Codex Plan"))
        .unwrap();
    assert_eq!(dividers.len(), 2, "{rendered}");
    assert!(
        dividers[0] < pi && pi < dividers[1] && dividers[1] < codex,
        "{rendered}"
    );
}

#[test]
fn usage_defaults_show_plan_days_remaining_time_and_day_blocks() {
    let sidebar =
        Sidebar::from_toml("modules = [\"usage\"]\n[usage]\nproviders = [\"codex\"]").unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let usage = UsageRow {
        provider: "codex".into(),
        display_name: "Codex Plan".into(),
        windows: vec![UsageWindow {
            label: "168h".into(),
            used_percent: 78.0,
            duration_seconds: Some(7 * 86_400),
            reset_at: Some(now + 3 * 86_400 + 13 * 3600),
        }],
        stale: false,
        fetched_at: Some(now * 1000),
        unavailable: false,
    };
    let rendered = sidebar
        .render_with_usage(&snapshot(), &[], &[usage])
        .unwrap();
    assert!(rendered.contains(" 7d 3d"), "{rendered}");
    assert_eq!(rendered.matches('█').count(), 5, "{rendered}");
    assert!(rendered.contains('▄'), "{rendered}");
    assert!(rendered.contains("78%"), "{rendered}");
    assert!(!rendered.contains("168h"), "{rendered}");
}

#[test]
fn low_usage_uses_a_bottom_fill_over_the_empty_bar_track() {
    let sidebar =
        Sidebar::from_toml("modules = [\"usage\"]\n[usage]\nproviders = [\"codex\"]").unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let usage = UsageRow {
        provider: "codex".into(),
        display_name: "Codex Plan".into(),
        windows: vec![UsageWindow {
            label: "168h".into(),
            used_percent: 1.0,
            duration_seconds: Some(7 * 86_400),
            reset_at: Some(now + 6 * 86_400 + 23 * 3600),
        }],
        stale: false,
        fetched_at: Some(now * 1000),
        unavailable: false,
    };
    let rendered = sidebar
        .render_with_usage(&snapshot(), &[], &[usage])
        .unwrap();
    assert!(rendered.contains("#[default,bg=colour238]▁"), "{rendered}");
    assert_eq!(
        rendered.matches("#[bg=colour238] ").count(),
        6,
        "{rendered}"
    );
    assert!(
        !rendered.contains("fg=colour208") && !rendered.contains("fg=red"),
        "{rendered}"
    );
    assert!(rendered.contains("1%"), "{rendered}");
}

#[test]
fn default_usage_columns_align_across_providers() {
    let sidebar = Sidebar::from_toml(
        "modules = [\"usage\"]\n[usage]\nproviders = [\"codex\", \"anthropic\", \"copilot\"]",
    )
    .unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let usage = |provider: &str,
                 label: &str,
                 percent: f64,
                 duration_seconds: Option<u64>,
                 reset_at: Option<u64>| UsageRow {
        provider: provider.into(),
        display_name: provider.into(),
        windows: vec![UsageWindow {
            label: label.into(),
            used_percent: percent,
            duration_seconds,
            reset_at,
        }],
        stale: false,
        fetched_at: Some(now * 1000),
        unavailable: false,
    };
    let rendered = sidebar
        .render_with_usage(
            &snapshot(),
            &[],
            &[
                usage(
                    "codex",
                    "168h",
                    78.0,
                    Some(7 * 86_400),
                    Some(now + 3 * 86_400),
                ),
                usage("anthropic", "5h", 42.0, Some(5 * 3600), None),
                usage("copilot", "Month", 100.0, None, Some(now + 4 * 86_400)),
            ],
        )
        .unwrap();
    let visible = |row: &str| {
        let mut result = String::new();
        let mut rest = row;
        while let Some(start) = rest.find("#[") {
            result.push_str(&rest[..start]);
            let end = rest[start..].find(']').unwrap();
            rest = &rest[start + end + 1..];
        }
        result.push_str(rest);
        result
    };
    let lines: Vec<_> = rendered
        .split("#[nl]")
        .map(visible)
        .filter(|line| line.contains('%'))
        .collect();
    assert_eq!(lines.len(), 3, "{rendered}");
    let bar_column = lines[0]
        .chars()
        .position(|character| character == '█')
        .unwrap();
    let percent_end = lines[0]
        .chars()
        .position(|character| character == '%')
        .unwrap();
    for line in &lines {
        assert_eq!(
            line.chars().position(|character| character == '█'),
            Some(bar_column),
            "{lines:?}"
        );
        assert_eq!(
            line.chars().position(|character| character == '%'),
            Some(percent_end),
            "{lines:?}"
        );
        assert!(line.chars().count() <= 30, "{lines:?}");
    }
}

#[test]
fn usage_bars_signal_high_usage_and_insufficient_quota_for_remaining_days() {
    let sidebar = Sidebar::from_toml(
        r##"
modules = ["usage"]
palette = "tokyo"
[palettes.tokyo]
text = "#82aaff"
orange = "#ff9e64"
danger = "#f7768e"
[usage]
providers = ["codex"]
window_style = "fg=text"
warning_bar_style = "fg=orange"
critical_bar_style = "fg=danger"
"##,
    )
    .unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let usage = |percent, reset_at| UsageRow {
        provider: "codex".into(),
        display_name: "Codex Plan".into(),
        windows: vec![UsageWindow {
            label: "168h".into(),
            used_percent: percent,
            duration_seconds: Some(7 * 86_400),
            reset_at,
        }],
        stale: false,
        fetched_at: Some(now * 1000),
        unavailable: false,
    };
    let render = |percent, reset_at| {
        sidebar
            .render_with_usage(&snapshot(), &[], &[usage(percent, reset_at)])
            .unwrap()
    };
    let warning = render(70.0, Some(now + 4 * 86_400));
    assert!(warning.contains("#[fg=#ff9e64,bg=colour238]"), "{warning}");
    assert!(
        warning.contains("#[fg=#82aaff,bg=default] 70%"),
        "{warning}"
    );
    let critical = render(80.0, Some(now + 4 * 86_400));
    assert!(
        critical.contains("#[fg=#f7768e,bg=colour238]"),
        "{critical}"
    );
    assert!(!critical.contains("fg=#ff9e64"), "{critical}");
    let normal = render(30.0, Some(now + 4 * 86_400));
    assert!(
        !normal.contains("fg=#ff9e64") && !normal.contains("fg=#f7768e"),
        "{normal}"
    );
    let unknown_reset = render(70.0, None);
    assert!(!unknown_reset.contains("fg=#ff9e64"), "{unknown_reset}");
}

#[test]
fn usage_rows_follow_configured_order_and_escape_provider_data() {
    let sidebar = Sidebar::from_toml(
        r##"
modules = ["usage", "divider"]
palette = "mine"
[palettes.mine]
muted = "#828bb8"
[usage]
providers = ["codex", "anthropic"]
format = "  $name $bar $percent"
stale_style = "fg=muted"
"##,
    )
    .unwrap();
    let usage = |provider: &str, display_name: &str, stale: bool, percent: f64| UsageRow {
        provider: provider.into(),
        display_name: display_name.into(),
        windows: vec![UsageWindow {
            label: "#[range=user|bad]#{oops}".into(),
            used_percent: percent,
            duration_seconds: None,
            reset_at: None,
        }],
        stale,
        fetched_at: Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64
                - if stale { 31 * 60_000 } else { 60_000 },
        ),
        unavailable: false,
    };
    let mut input = snapshot();
    input.width = 50;
    let rendered = sidebar
        .render_with_usage(
            &input,
            &[],
            &[
                usage("anthropic", "Claude", true, 60.0),
                usage("codex", "Codex", false, 30.0),
            ],
        )
        .unwrap();
    assert!(rendered.find("Codex").unwrap() < rendered.find("Claude").unwrap());
    assert!(rendered.find("Claude").unwrap() < rendered.find("----------").unwrap());
    assert!(rendered.contains("Claude (31m old)"), "{rendered}");
    assert!(!rendered.contains("Codex ("), "{rendered}");
    assert!(rendered.contains("#[fg=#828bb8,bg=default]"), "{rendered}");
    assert!(
        rendered.contains("##[range=user|bad]##{oops}"),
        "{rendered}"
    );
    assert!(rendered.contains("███"), "{rendered}");
    assert!(rendered.contains("#[bg=colour238]       "), "{rendered}");
    assert!(rendered.contains("30%"), "{rendered}");
    assert!(!rendered.contains("#[range=user|bad]#{oops}"));

    let unavailable = UsageRow {
        provider: "codex".into(),
        display_name: "Codex".into(),
        windows: Vec::new(),
        stale: false,
        fetched_at: None,
        unavailable: true,
    };
    let unavailable_rendered = sidebar
        .render_with_usage(&input, &[], &[unavailable])
        .unwrap();
    assert!(unavailable_rendered.contains("Codex (unavailable)"));
    assert!(!unavailable_rendered.contains("#[range=user|"));

    input.width = 12;
    let clipped = sidebar
        .render_with_usage(&input, &[], &[usage("codex", "Codex", false, 50.0)])
        .unwrap();
    assert!(!clipped.contains("#{oops}"));
}

#[test]
fn usage_module_can_be_disabled_and_rejects_invalid_settings() {
    let disabled = Sidebar::from_toml("modules = [\"usage\"]\n[usage]\ndisabled = true").unwrap();
    let rendered = disabled.render(&snapshot()).unwrap();
    assert_eq!(rendered.matches("#[nl]").count(), 2);
    for config in [
        "modules = [\"usage\", \"usage\"]",
        "[usage]\nproviders = [\"unknown\"]",
        "[usage]\nproviders = [\"codex\", \"codex\"]",
        "[usage]\ncache_dir = \"relative\"",
        "[usage]\nformat = \"$unsafe\"",
        "[usage]\nprovider_style = \"fg=#[bad]\"",
    ] {
        assert!(Sidebar::from_toml(config).is_err(), "accepted {config}");
    }
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
