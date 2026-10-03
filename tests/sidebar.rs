use starmux::usage::{RefreshFailure, UsageRow, UsageWindow};
use starmux::{PiSession, PiTarget, Session, Sidebar, Snapshot, Window};
use std::collections::BTreeMap;

fn snapshot() -> Snapshot {
    Snapshot {
        width: 30,
        client_width: 100,
        client_height: 25,
        status_lines: 1,
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
    assert!(rendered.contains("#[range=user|st0 "));
    assert!(rendered.contains("#[range=user|sw0 list=focus "));
    assert!(rendered.contains("#[range=user|sw1z141z6 "));
    assert!(rendered.contains("##[fg=red] x##{oops}"));
    assert!(rendered.contains("----------------------------"));
    assert!(!rendered.contains("●"));
    assert!(!rendered.contains("#c099ff"));
}

#[test]
fn active_only_sessions_follow_focus_and_round_trip() {
    let sidebar = Sidebar::from_toml("[sessions]\nactive_only = true").unwrap();
    let printed = sidebar.print_config().unwrap();
    assert!(printed.contains("active_only = true"));
    let sidebar = Sidebar::from_toml(&printed).unwrap();
    let mut input = snapshot();
    let rendered = sidebar.render(&input).unwrap();
    assert!(rendered.contains("range=user|st0 "));
    assert!(rendered.contains("range=user|sw0 list=focus "));
    assert!(rendered.contains("range=user|sw1 "));
    assert!(rendered.contains("##[fg=red] x##{oops}"));
    assert!(!rendered.contains("range=user|st1 "));
    assert!(!rendered.contains("range=user|sw1z141z6 "));

    input.current_session = "$1".into();
    input.current_pane = "%2".into();
    let rendered = sidebar.render(&input).unwrap();
    assert!(rendered.contains("range=user|st1 "));
    assert!(rendered.contains("range=user|sw1z141z6 list=focus "));
    assert!(!rendered.contains("range=user|st0 "));
    assert!(!rendered.contains("range=user|sw0 "));
    assert!(!rendered.contains("range=user|sw1 "));

    let all = Sidebar::from_toml("[sessions]\nactive_only = false").unwrap();
    assert_eq!(
        all.render(&input).unwrap(),
        Sidebar::defaults().unwrap().render(&input).unwrap()
    );
}

#[test]
fn active_only_respects_layout_visibility_named_lists_and_folding() {
    for (settings, width, windows) in [
        ("", 30, true),
        ("show_windows = false", 30, false),
        ("", 2, false),
        ("[slim]\nshow_windows = true", 2, true),
        ("show_windows = false\n[slim]\nshow_windows = true", 2, true),
    ] {
        let config = format!(
            "[sessions]\nactive_only = true\n{settings}\n[configs.right]\nmodules = ['sessions']"
        );
        let sidebar = Sidebar::from_toml(&config).unwrap();
        let folded = Sidebar::from_toml(&config.replace(
            "active_only = true",
            "active_only = true\nfold_inactive = true",
        ))
        .unwrap();
        let mut input = snapshot();
        input.width = width;
        for (sidebar, folded) in [
            (sidebar.clone(), folded.clone()),
            (
                sidebar.select("right").unwrap(),
                folded.select("right").unwrap(),
            ),
        ] {
            let rendered = sidebar.render(&input).unwrap();
            assert_eq!(rendered, folded.render(&input).unwrap());
            assert!(!rendered.contains("range=user|st1 "));
            assert!(!rendered.contains("range=user|sw1z141z6 "));
            if windows {
                assert!(rendered.contains("range=user|sw0 list=focus "));
                assert!(rendered.contains("range=user|sw1 "));
            } else {
                assert!(rendered.contains("range=user|st0 list=focus "));
                assert!(!rendered.contains("range=user|sw"));
            }
        }
    }
}

#[test]
fn folding_inactive_sessions_follows_tmux_focus_and_preserves_headings() {
    let sidebar = Sidebar::from_toml("[sessions]\nfold_inactive = true").unwrap();
    let printed = sidebar.print_config().unwrap();
    assert!(printed.contains("fold_inactive = true"));
    let sidebar = Sidebar::from_toml(&printed).unwrap();
    let mut input = snapshot();
    let rendered = sidebar.render(&input).unwrap();
    assert!(rendered.contains("range=user|st0 "));
    assert!(rendered.contains("range=user|st1 "));
    assert!(rendered.contains("range=user|sw0 list=focus "));
    assert!(rendered.contains("range=user|sw1 "));
    assert!(rendered.contains("##[fg=red] x##{oops}"));
    assert!(!rendered.contains("range=user|sw1z141z6 "));

    input.current_session = "$1".into();
    input.current_pane = "%2".into();
    let rendered = sidebar.render(&input).unwrap();
    assert!(rendered.contains("range=user|st0 "));
    assert!(rendered.contains("range=user|st1 "));
    assert!(rendered.contains("range=user|sw1z141z6 list=focus "));
    assert!(!rendered.contains("range=user|sw0 "));
    assert!(!rendered.contains("range=user|sw1 "));

    let unfolded = Sidebar::from_toml("[sessions]\nfold_inactive = false").unwrap();
    assert_eq!(
        unfolded.render(&input).unwrap(),
        Sidebar::defaults().unwrap().render(&input).unwrap()
    );
}

#[test]
fn folding_respects_full_and_slim_window_visibility_and_named_lists() {
    for (settings, width, windows) in [
        ("", 30, true),
        ("show_windows = false", 30, false),
        ("", 2, false),
        ("[slim]\nshow_windows = true", 2, true),
        ("show_windows = false\n[slim]\nshow_windows = true", 2, true),
    ] {
        let config = format!(
            "[sessions]\nfold_inactive = true\n{settings}\n[configs.right]\nmodules = ['sessions']"
        );
        let sidebar = Sidebar::from_toml(&config).unwrap();
        let mut input = snapshot();
        input.width = width;
        for sidebar in [sidebar.clone(), sidebar.select("right").unwrap()] {
            let rendered = sidebar.render(&input).unwrap();
            assert!(rendered.contains("range=user|st1 "));
            assert!(!rendered.contains("range=user|sw1z141z6 "));
            if windows {
                assert!(rendered.contains("range=user|sw0 list=focus "));
                assert!(rendered.contains("range=user|sw1 "));
            } else {
                assert!(rendered.contains("range=user|st0 list=focus "));
                assert!(!rendered.contains("range=user|sw"));
            }
        }
    }
}

#[test]
fn folded_scroll_bounds_follow_content_without_changing_expansion() {
    let sidebar = Sidebar::from_toml(
        "modules = ['sessions', 'spacer', 'divider', 'pi-workbench']\n[sessions]\nfold_inactive = true",
    ).unwrap();
    let pi = PiSession {
        name: "working session".into(),
        location: None,
        state: "working".into(),
        selected: false,
        project: "project".into(),
        target: None,
    };
    let commands = BTreeMap::new();
    let mut input = snapshot();
    let mut render = |height, pi_sessions, offset| {
        input.client_height = height;
        sidebar
            .render_scrolled(
                &input,
                starmux::RenderInputs {
                    top: None,
                    pi_sessions,
                    usage_rows: &[],
                    gob_jobs: &[],
                    context: None,
                    states: &[],
                    commands: &commands,
                    git: None,
                    debug: None,
                },
                offset,
            )
            .unwrap()
    };
    for pi_sessions in [&[][..], std::slice::from_ref(&pi)] {
        let (rendered, offset, max) = render(20, pi_sessions, usize::MAX);
        assert_eq!((offset, max), (0, 0));
        assert!(rendered.contains("range=user|st1 "));
        assert!(rendered.contains("range=user|sw0 list=focus "));
        assert!(rendered.contains("range=user|sw1 "));
        assert!(!rendered.contains("range=user|sw1z141z6 "));
    }
    // Four navigation rows and one divider in a four-row viewport.
    let (_, offset, max) = render(5, &[], usize::MAX);
    assert_eq!((offset, max), (1, 1));
    // Pi section and project headings plus a session add three rows.
    let (_, offset, max) = render(5, std::slice::from_ref(&pi), usize::MAX);
    assert_eq!((offset, max), (4, 4));
    let (_, offset, max) = render(5, &[], 3);
    assert_eq!((offset, max), (1, 1));
}

#[test]
fn spacer_places_following_rows_at_the_bottom_and_collapses_on_overflow() {
    let sidebar = Sidebar::from_toml(
        "modules = [\"sessions\", \"gob\", \"spacer\", \"divider\"]\n[gob]\ndisabled = true",
    )
    .unwrap();
    let mut input = snapshot();
    input.client_height = 12;
    let rendered = sidebar.render(&input).unwrap();
    let lines: Vec<_> = rendered.split("#[nl]").collect();
    assert_eq!(lines.len() - 1, 13); // 11 visible rows plus two list marker newlines.
    assert!(lines[lines.len() - 2].contains("----------------------------"));
    let gap = &lines[7..lines.len() - 2];
    assert_eq!(gap.len(), 5);
    assert!(gap.iter().all(|line| line.contains("#[range=user|sv ")
        && !line.contains("#[range=user|st")
        && !line.contains("#[range=user|sw")));

    input.client_height = 5;
    let overfull = sidebar.render(&input).unwrap();
    let unspaced = Sidebar::from_toml("modules = [\"sessions\", \"divider\"]")
        .unwrap()
        .render(&input)
        .unwrap();
    assert_eq!(overfull, unspaced);
    assert!(Sidebar::from_toml("modules = [\"spacer\", \"spacer\"]").is_err());
}

#[test]
fn scrolling_pages_through_all_modules_and_clamps_after_resize() {
    let sidebar =
        Sidebar::from_toml("modules = [\"sessions\", \"spacer\", \"divider\", \"blank\"]").unwrap();
    let mut input = snapshot();
    input.client_height = 4;
    let mut render = |height, offset| {
        input.client_height = height;
        sidebar
            .render_scrolled(
                &input,
                starmux::RenderInputs {
                    top: None,
                    pi_sessions: &[],
                    usage_rows: &[],
                    gob_jobs: &[],
                    context: None,
                    states: &[],
                    commands: &BTreeMap::new(),
                    git: None,
                    debug: None,
                },
                offset,
            )
            .unwrap()
    };
    let (top, offset, max) = render(4, 0);
    assert_eq!(max, 4);
    assert_eq!(offset, 0);
    assert!(top.contains("#[range=user|st0 "));
    assert!(!top.contains("----------------------------"));

    let (middle, offset, _) = render(4, 2);
    assert_eq!(offset, 2);
    assert!(!middle.contains("#[range=user|st0 "));
    assert!(middle.contains("#[range=user|sw1z141z6 "));

    let (bottom, offset, _) = render(4, usize::MAX);
    assert_eq!(offset, 4);
    assert!(bottom.contains("----------------------------"));
    assert!(bottom.contains("#[range=user|sv "));
    assert!(!bottom.contains("#[range=user|st0 "));

    let (resized, offset, _) = render(8, usize::MAX);
    assert_eq!(offset, 0);
    assert!(resized.contains("#[range=user|st0 "));
    assert!(resized.contains("----------------------------"));
}

#[test]
fn blank_reserves_one_row_below_bottom_aligned_content() {
    let sidebar =
        Sidebar::from_toml("modules = [\"sessions\", \"spacer\", \"divider\", \"blank\"]").unwrap();
    let mut input = snapshot();
    input.client_height = 12;
    let rendered = sidebar.render(&input).unwrap();
    let lines: Vec<_> = rendered.split("#[nl]").collect();
    assert_eq!(lines.len() - 1, 13);
    assert!(lines[lines.len() - 3].contains("----------------------------"));
    assert!(lines[lines.len() - 2].contains("#[range=user|sv "));

    let repeated = Sidebar::from_toml("modules = [\"blank\", \"blank\"]")
        .unwrap()
        .render(&input)
        .unwrap();
    assert_eq!(repeated.matches("#[nl]").count(), 4);
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
    let session = rendered.find("#[range=user|st0 ").unwrap();
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
fn pi_workbench_config_replaces_pi_live() {
    let sidebar =
        Sidebar::from_toml("modules = [\"pi-workbench\"]\n[pi-workbench]\nformat = \"$name\"")
            .unwrap();
    let printed = sidebar.print_config().unwrap();
    assert!(printed.contains("\"pi-workbench\""));
    assert!(printed.contains("[pi-workbench]"));

    assert!(Sidebar::from_toml("modules = [\"pi-live\"]").is_err());
    assert!(Sidebar::from_toml("[pi-live]\nformat = \"$name\"").is_err());
}

#[test]
fn pi_workbench_rows_follow_module_order_and_escape_session_names() {
    let sidebar = Sidebar::from_toml(
        r##"
modules = ["pi-workbench", "divider"]
palette = "tokyo"
[palettes.tokyo]
warning = "#ffc777"
muted = "#828bb8"
[pi-workbench]
format = " $state $name"
working_style = "fg=warning"
idle_style = "fg=muted"
"##,
    )
    .unwrap();
    let mut input = snapshot();
    input.width = 50;
    let rendered = sidebar
        .render_with_pi_workbench(
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
    assert_eq!(rendered.matches("π Sessions").count(), 1);
    assert!(rendered.find("π Sessions").unwrap() < rendered.find("starmux").unwrap());
    assert!(rendered.find("waiting").unwrap() < rendered.find("----------").unwrap());
    assert!(!sidebar.render(&input).unwrap().contains("π Sessions"));
    let pi = PiSession {
        state: "idle".into(),
        project: "/projects/starmux".into(),
        name: "only session".into(),
        location: None,
        target: None,
        selected: false,
    };
    let single = sidebar
        .render_with_pi_workbench(&input, std::slice::from_ref(&pi))
        .unwrap();
    assert_eq!(single.matches("π Sessions").count(), 1);
    input.width = 2;
    let slim = sidebar.render_with_pi_workbench(&input, &[pi]).unwrap();
    assert!(!slim.contains("π Sessions"));
    assert!(!rendered.contains("#[range=user|bad]#{oops}"));
    assert!(!rendered.contains("#[range=window|"));

    for config in [
        "modules = [\"pi-workbench\"]\n[pi-workbench]\nformat = \"$cwd\"",
        "modules = [\"pi-workbench\"]\n[pi-workbench]\nworking_style = \"fg=#[bad]\"",
        "modules = [\"pi-workbench\"]\n[pi-workbench]\nheading_style = \"fg=#[bad]\"",
        "modules = [\"pi-workbench\"]\n[pi-workbench]\ndata_dir = \"relative\"",
    ] {
        assert!(Sidebar::from_toml(config).is_err(), "accepted {config}");
    }
}

#[test]
fn pi_projects_stay_together_in_alphabetical_order() {
    let sidebar = Sidebar::from_toml("modules = [\"pi-workbench\"]").unwrap();
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
        .render_with_pi_workbench(
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
        "later idle",
        "needs attention",
        "#[bold] idle",
        "only idle",
        "#[bold] selected",
        "current idle",
        "#[bold] working",
        "doing work",
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
        .render_with_pi_workbench(
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
fn pi_order_survives_state_selection_and_input_order_changes_in_both_layouts() {
    let sidebar = Sidebar::from_toml(
        r#"
modules = ["pi-workbench"]
[pi-workbench]
selected_fill = "blue"
selected_style = "fg=white,bold"
idle_style = "fg=green"
working_style = "fg=yellow"
notify_style = "fg=magenta"
"#,
    )
    .unwrap();
    let session = |project: &str, name: &str, pane: &str| PiSession {
        name: name.into(),
        project: format!("/projects/{project}"),
        state: "idle".into(),
        location: Some(starmux::PiLocation {
            pane: pane.into(),
            session_name: "main".into(),
        }),
        target: Some(PiTarget {
            pane: pane.into(),
            window: "@0".into(),
        }),
        selected: false,
    };
    let sessions = [
        session("zebra", "beta", "%4"),
        session("app", "alpha", "%3"),
        session("app", "Alpha", "%2"),
        session("app", "alpha", "%1"),
    ];
    let token = |row: &str| {
        row.split("#[range=user|")
            .nth(1)
            .unwrap()
            .split(' ')
            .next()
            .unwrap()
            .to_owned()
    };
    let expected_sessions = [2, 3, 1, 0];
    let expected_tokens: Vec<_> = expected_sessions
        .iter()
        .map(|&index| {
            let rendered = sidebar
                .render_with_pi_workbench(&snapshot(), &sessions[index..=index])
                .unwrap();
            token(
                rendered
                    .split("#[nl]")
                    .find(|row| row.contains("#[range=user|sp"))
                    .unwrap(),
            )
        })
        .collect();
    for width in [30, 2] {
        let mut input = snapshot();
        input.width = width;
        for selected in 0..sessions.len() {
            let mut changed = sessions.clone();
            for (index, session) in changed.iter_mut().enumerate() {
                session.state = ["idle", "working", "notify"][(index + selected) % 3].into();
                session.selected = index == selected;
            }
            if selected % 2 == 0 {
                changed.reverse();
            }
            let rendered = sidebar.render_with_pi_workbench(&input, &changed).unwrap();
            let rows: Vec<_> = rendered
                .split("#[nl]")
                .filter(|row| row.contains("#[range=user|sp"))
                .collect();
            assert_eq!(
                rows.iter().map(|row| token(row)).collect::<Vec<_>>(),
                expected_tokens
            );
            for (row, &index) in rows.iter().zip(&expected_sessions) {
                let (color, glyph) = match (index + selected) % 3 {
                    0 => ("fg=green", "○"),
                    1 => ("fg=yellow", "▶"),
                    _ => ("fg=magenta", "󰂚"),
                };
                assert!(row.contains(color), "{row}");
                assert_eq!(row.contains("bg=blue"), index == selected, "{row}");
                if width == 2 {
                    assert!(row.contains(glyph), "{row}");
                } else {
                    assert!(row.contains(&sessions[index].name), "{row}");
                    assert!(row.contains('●'), "{row}");
                }
            }
        }
    }
}

#[test]
fn selected_pi_background_does_not_fill_following_read_only_row() {
    let sidebar = Sidebar::from_toml(
        r##"
modules = ["pi-workbench"]
palette = "tokyo"
[palettes.tokyo]
accent = "#c099ff"
border = "#3b4261"
[pi-workbench]
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
            project: "/projects/deltoids".into(),
            state: "idle".into(),
            location: None,
            target: None,
            selected: false,
        },
    ];
    let rendered = sidebar
        .render_with_pi_workbench(&snapshot(), &rows)
        .unwrap();
    let lines: Vec<_> = rendered.split("#[nl]").collect();
    let selected = lines.iter().find(|line| line.contains("selected")).unwrap();
    let read_only = lines
        .iter()
        .find(|line| line.contains("read only"))
        .unwrap();
    assert!(selected.contains("#[fill=#3b4261]"), "{rendered}");
    assert!(read_only.contains("read only#[bg=default]"), "{rendered}");
    assert!(
        read_only.contains("#[norange default]#[fill=default]"),
        "{rendered}"
    );
}

#[test]
fn empty_modules_leave_one_divider_between_visible_sections() {
    let sidebar = Sidebar::from_toml(
        "modules = [\"sessions\", \"divider\", \"pi-workbench\", \"divider\", \"usage\"]\n[divider]\ncharacter = \"=\"",
    )
    .unwrap();
    let input = snapshot();
    let empty = sidebar.render(&input).unwrap();
    assert_eq!(empty.matches("============================").count(), 1);

    let pi = PiSession {
        state: "idle".into(),
        project: "/projects/starmux".into(),
        name: "pi session".into(),
        location: None,
        target: None,
        selected: false,
    };
    let populated = sidebar.render_with_pi_workbench(&input, &[pi]).unwrap();
    assert_eq!(populated.matches("============================").count(), 2);
    let first = populated.find("============================").unwrap();
    let pi_row = populated.find("pi session").unwrap();
    let second = populated.rfind("============================").unwrap();
    assert!(first < pi_row && pi_row < second);
}

#[test]
fn single_dividers_and_blank_rows_remain_visible() {
    let input = snapshot();
    for modules in [
        "[\"pi-workbench\", \"divider\"]",
        "[\"divider\", \"pi-workbench\"]",
        "[\"divider\", \"blank\", \"divider\"]",
    ] {
        let config = format!("modules = {modules}\n[divider]\ncharacter = \"=\"");
        let rendered = Sidebar::from_toml(&config).unwrap().render(&input).unwrap();
        let expected = if modules.contains("blank") { 2 } else { 1 };
        assert_eq!(
            rendered.matches("============================").count(),
            expected
        );
    }
    let repeated = Sidebar::from_toml(
        "modules = [\"divider\", \"divider\", \"divider\"]\n[divider]\ncharacter = \"=\"",
    )
    .unwrap()
    .render(&input)
    .unwrap();
    assert_eq!(repeated.matches("============================").count(), 1);
}

#[test]
fn collapsed_dividers_keep_bottom_alignment_and_scroll_bounds() {
    let sidebar = Sidebar::from_toml(
        "modules = [\"sessions\", \"divider\", \"pi-workbench\", \"divider\", \"spacer\", \"debug\"]\n[divider]\ncharacter = \"=\"",
    )
    .unwrap();
    let mut input = snapshot();
    input.client_height = 12;
    let rendered = sidebar.render(&input).unwrap();
    let lines: Vec<_> = rendered.split("#[nl]").collect();
    assert_eq!(lines.len() - 1, 13);
    assert_eq!(rendered.matches("============================").count(), 1);
    assert!(lines[lines.len() - 2].contains("last --"));

    input.client_height = 7;
    let (_, offset, max) = sidebar
        .render_scrolled(
            &input,
            starmux::RenderInputs {
                top: None,
                pi_sessions: &[],
                usage_rows: &[],
                gob_jobs: &[],
                context: None,
                states: &[],
                commands: &BTreeMap::new(),
                git: None,
                debug: None,
            },
            usize::MAX,
        )
        .unwrap();
    assert_eq!(max, 1);
    assert_eq!(offset, max);
}

#[test]
fn command_output_separates_dividers_in_a_named_list() {
    let sidebar = Sidebar::from_toml(
        "modules = []\n[configs.right]\nmodules = [\"divider\", \"command.build\", \"divider\"]\n[commands.build]\nargv = [\"echo\", \"ready\"]\n[divider]\ncharacter = \"=\"",
    )
    .unwrap()
    .select("right")
    .unwrap();
    let input = snapshot();
    let empty = sidebar.render(&input).unwrap();
    assert_eq!(empty.matches("============================").count(), 1);
    let commands = BTreeMap::from([("command.build".into(), "ready".into())]);
    let rendered = sidebar
        .render_with_inputs(
            &input,
            starmux::RenderInputs {
                top: None,
                pi_sessions: &[],
                usage_rows: &[],
                gob_jobs: &[],
                context: None,
                states: &[],
                commands: &commands,
                git: None,
                debug: None,
            },
        )
        .unwrap();
    assert_eq!(rendered.matches("============================").count(), 2);
    assert!(
        rendered.find("============================").unwrap() < rendered.find("ready").unwrap()
    );
    assert!(
        rendered.find("ready").unwrap() < rendered.rfind("============================").unwrap()
    );
}

#[test]
fn spacer_only_separates_dividers_when_it_has_room() {
    let sidebar = Sidebar::from_toml(
        "modules = [\"sessions\", \"divider\", \"spacer\", \"divider\", \"debug\"]\n[divider]\ncharacter = \"=\"",
    )
    .unwrap();
    let mut input = snapshot();
    input.client_height = 7;
    let full = sidebar.render(&input).unwrap();
    assert_eq!(full.matches("============================").count(), 1);
    input.client_height = 10;
    let spaced = sidebar.render(&input).unwrap();
    assert_eq!(spaced.matches("============================").count(), 2);
    assert_eq!(spaced.matches("#[nl]").count(), 11);
}

#[test]
fn divider_can_separate_usage_from_pi_sessions() {
    let sidebar = Sidebar::from_toml(
        r#"
modules = ["sessions", "divider", "pi-workbench", "divider", "usage"]
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
        available_resets: None,
        windows: vec![UsageWindow {
            label: "5h".into(),
            used_percent: 40.0,
            duration_seconds: None,
            reset_at: None,
        }],
        stale: false,
        fetched_at: Some(100),
        unavailable: false,
        refresh_failure: None,
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
        available_resets: None,
        windows: vec![UsageWindow {
            label: "168h".into(),
            used_percent: 78.0,
            duration_seconds: Some(7 * 86_400),
            reset_at: Some(now + 3 * 86_400 + 13 * 3600),
        }],
        stale: false,
        fetched_at: Some(now * 1000),
        unavailable: false,
        refresh_failure: None,
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
        available_resets: None,
        windows: vec![UsageWindow {
            label: "168h".into(),
            used_percent: 1.0,
            duration_seconds: Some(7 * 86_400),
            reset_at: Some(now + 6 * 86_400 + 23 * 3600),
        }],
        stale: false,
        fetched_at: Some(now * 1000),
        unavailable: false,
        refresh_failure: None,
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
        available_resets: None,
        windows: vec![UsageWindow {
            label: label.into(),
            used_percent: percent,
            duration_seconds,
            reset_at,
        }],
        stale: false,
        fetched_at: Some(now * 1000),
        unavailable: false,
        refresh_failure: None,
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
        available_resets: None,
        windows: vec![UsageWindow {
            label: "168h".into(),
            used_percent: percent,
            duration_seconds: Some(7 * 86_400),
            reset_at,
        }],
        stale: false,
        fetched_at: Some(now * 1000),
        unavailable: false,
        refresh_failure: None,
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
        available_resets: None,
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
        refresh_failure: None,
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
        available_resets: None,
        windows: Vec::new(),
        stale: false,
        fetched_at: None,
        unavailable: true,
        refresh_failure: None,
    };
    let unavailable_rendered = sidebar
        .render_with_usage(&input, &[], &[unavailable])
        .unwrap();
    assert!(unavailable_rendered.contains("Codex (unavailable)"));
    assert!(unavailable_rendered.contains("#[range=user|su4 ]"));

    input.width = 12;
    let clipped = sidebar
        .render_with_usage(&input, &[], &[usage("codex", "Codex", false, 50.0)])
        .unwrap();
    assert!(!clipped.contains("#{oops}"));
}

#[test]
fn codex_heading_shows_available_resets() {
    let sidebar =
        Sidebar::from_toml("modules = [\"usage\"]\n[usage]\nproviders = [\"codex\"]").unwrap();
    let mut input = snapshot();
    input.width = 60;
    let mut usage = UsageRow {
        provider: "codex".into(),
        display_name: "Codex Plan".into(),
        available_resets: Some(1),
        windows: Vec::new(),
        stale: false,
        fetched_at: None,
        unavailable: false,
        refresh_failure: None,
    };
    for (count, text) in [
        (Some(1), " · 1 reset"),
        (Some(0), " · 0 resets"),
        (Some(2), " · 2 resets"),
        (None, "Codex Plan"),
    ] {
        usage.available_resets = count;
        let rendered = sidebar
            .render_with_usage(&input, &[], &[usage.clone()])
            .unwrap();
        assert!(rendered.contains("Codex Plan"), "{rendered}");
        assert!(rendered.contains(text), "{rendered}");
        if count.is_none() {
            assert!(!rendered.contains(" · "), "{rendered}");
        } else {
            assert!(rendered.contains("#[dim,bg=default] · "), "{rendered}");
        }
        assert!(rendered.contains("#[range=user|su4 ]"), "{rendered}");
    }
    usage.available_resets = Some(1);
    usage.stale = true;
    usage.fetched_at = Some(0);
    let rendered = sidebar.render_with_usage(&input, &[], &[usage]).unwrap();
    assert!(rendered.contains(" · 1 reset"), "{rendered}");
    assert!(rendered.contains(" old)"), "{rendered}");
}

#[test]
fn failed_codex_refresh_keeps_cached_percentage_with_a_visible_status() {
    let sidebar =
        Sidebar::from_toml("modules = [\"usage\"]\n[usage]\nproviders = [\"codex\"]").unwrap();
    let mut input = snapshot();
    input.width = 70;
    let mut usage = UsageRow {
        provider: "codex".into(),
        display_name: "Codex Plan".into(),
        available_resets: Some(2),
        windows: vec![UsageWindow {
            label: "5h".into(),
            used_percent: 100.0,
            duration_seconds: Some(18_000),
            reset_at: None,
        }],
        stale: false,
        fetched_at: Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64
                - 60_000,
        ),
        unavailable: false,
        refresh_failure: Some(RefreshFailure::SignInAgain),
    };
    let failed = sidebar
        .render_with_usage(&input, &[], &[usage.clone()])
        .unwrap();
    assert!(failed.contains("Codex Plan (sign in again)"), "{failed}");
    assert!(failed.contains("cached 1m old"), "{failed}");
    assert!(failed.contains("100%"), "{failed}");
    assert!(!failed.contains(" · 2 resets"), "{failed}");
    assert!(failed.contains("#[range=user|su4 ]"), "{failed}");
    input.width = 30;
    let narrow = sidebar
        .render_with_usage(&input, &[], &[usage.clone()])
        .unwrap();
    assert!(narrow.contains("sign in again)"), "{narrow}");
    assert!(narrow.contains("cached 1m old"), "{narrow}");
    input.width = 70;

    usage.refresh_failure = None;
    usage.windows[0].used_percent = 2.0;
    let recovered = sidebar
        .render_with_usage(&input, &[], &[usage.clone()])
        .unwrap();
    assert!(!recovered.contains("sign in again"), "{recovered}");
    assert!(!recovered.contains("cached "), "{recovered}");
    assert!(recovered.contains("2%"), "{recovered}");

    usage.unavailable = true;
    usage.windows.clear();
    usage.refresh_failure = Some(RefreshFailure::RefreshFailed);
    let unavailable = sidebar.render_with_usage(&input, &[], &[usage]).unwrap();
    assert!(unavailable.contains("refresh failed"), "{unavailable}");
}

#[test]
fn usage_plan_rows_link_to_their_provider_page() {
    let sidebar =
        Sidebar::from_toml("modules = [\"usage\"]\n[usage]\nproviders = [\"codex\"]").unwrap();
    let usage = UsageRow {
        provider: "codex".into(),
        display_name: "#[range=user|su0]".into(),
        available_resets: None,
        windows: ["5h", "Week"]
            .into_iter()
            .map(|label| UsageWindow {
                label: label.into(),
                used_percent: 20.0,
                duration_seconds: None,
                reset_at: None,
            })
            .collect(),
        stale: true,
        fetched_at: None,
        unavailable: false,
        refresh_failure: None,
    };
    let rendered = sidebar
        .render_with_usage(&snapshot(), &[], &[usage])
        .unwrap();
    assert_eq!(
        rendered.matches("#[range=user|su4 ]").count(),
        3,
        "{rendered}"
    );
    assert!(rendered.contains("##[range=user|su0]"), "{rendered}");
    assert_eq!(rendered.matches("#[range=user|su0 ]").count(), 0);
}

#[test]
fn usage_without_a_web_page_has_no_click_target() {
    let sidebar =
        Sidebar::from_toml("modules = [\"usage\"]\n[usage]\nproviders = [\"gemini\"]").unwrap();
    let usage = UsageRow {
        provider: "gemini".into(),
        display_name: "Gemini".into(),
        available_resets: None,
        windows: vec![UsageWindow {
            label: "Pro".into(),
            used_percent: 20.0,
            duration_seconds: None,
            reset_at: None,
        }],
        stale: false,
        fetched_at: None,
        unavailable: false,
        refresh_failure: None,
    };
    let rendered = sidebar
        .render_with_usage(&snapshot(), &[], &[usage])
        .unwrap();
    assert!(rendered.contains("Gemini"));
    assert!(rendered.contains("#[range=user|sv "), "{rendered}");
    assert!(!rendered.contains("#[range=user|su"), "{rendered}");
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

#[test]
fn pi_context_open_command_requires_one_file_argument_and_separate_placeholders() {
    for command in [
        "[]",
        "[\"dev\", \"tmux\", \"edit\"]",
        "[\"{file}\"]",
        "[\"dev\", \"{file}\", \"{file}\"]",
        "[\"dev\", \"--file={file}\"]",
        "[\"dev\", \"{file}\", \"{unknown}\"]",
    ] {
        assert!(
            Sidebar::from_toml(&format!(
                "modules = [\"pi-context\"]\n[pi-context]\nopen_command = {command}"
            ))
            .is_err(),
            "{command}"
        );
    }
    assert!(Sidebar::from_toml("modules = [\"pi-context\"]\n[pi-context]\nopen_command = [\"dev\", \"tmux\", \"edit\", \"{file}\", \"{pane}\", \"{socket}\"]").is_ok());
}

#[test]
fn pi_context_names_the_selected_session_above_its_entries() {
    let sidebar = Sidebar::from_toml("modules = [\"pi-context\"]").unwrap();
    let context = starmux::PiContext {
        plans: vec![starmux::PiPlan {
            title: "Inspect hierarchy".into(),
            path: "/missing/plan.md".into(),
        }],
        skills: vec![starmux::PiSkill {
            name: "vocabulary".into(),
            path: None,
        }],
        ..Default::default()
    };
    let sessions = [
        PiSession {
            name: "another session".into(),
            project: "/project".into(),
            state: "idle".into(),
            location: None,
            target: None,
            selected: false,
        },
        PiSession {
            name: "Plan #[fg=red] dividers".into(),
            project: "/project".into(),
            state: "working".into(),
            location: None,
            target: None,
            selected: true,
        },
    ];
    let render = |context: Option<&starmux::PiContext>, pi_sessions: &[PiSession]| {
        sidebar
            .render_with_inputs(
                &snapshot(),
                starmux::RenderInputs {
                    top: None,
                    pi_sessions,
                    usage_rows: &[],
                    gob_jobs: &[],
                    context,
                    states: &[],
                    commands: &BTreeMap::new(),
                    git: None,
                    debug: None,
                },
            )
            .unwrap()
    };
    let rendered = render(Some(&context), &sessions);
    let lines: Vec<_> = rendered.split("#[nl]").collect();
    let title = lines
        .iter()
        .position(|line| line.contains(" π Plan ##[fg=red] dividers"))
        .unwrap();
    let plans = lines
        .iter()
        .position(|line| line.contains(" Plans"))
        .unwrap();
    let item = lines
        .iter()
        .position(|line| line.contains("Inspect hierarchy"))
        .unwrap();
    let skills = lines
        .iter()
        .position(|line| line.contains(" Skills"))
        .unwrap();
    assert!(title < plans && plans < item && item < skills, "{rendered}");
    assert!(!rendered.contains("another session"), "{rendered}");
    assert!(lines[title].contains("#[range=user|sv "), "{rendered}");
    assert!(render(Some(&context), &[]).contains(" π"));
    for width in [16, 24] {
        let mut narrow = snapshot();
        narrow.width = width;
        let output = sidebar
            .render_with_inputs(
                &narrow,
                starmux::RenderInputs {
                    top: None,
                    pi_sessions: &sessions,
                    usage_rows: &[],
                    gob_jobs: &[],
                    context: Some(&context),
                    states: &[],
                    commands: &BTreeMap::new(),
                    git: None,
                    debug: None,
                },
            )
            .unwrap();
        assert!(output.contains(" π Plan ##[fg=red"), "{output}");
    }
    assert!(!render(Some(&starmux::PiContext::default()), &sessions).contains(" π"));
    assert!(!render(None, &sessions).contains(" π"));
}

#[test]
fn pi_context_pr_state_icons_use_distinct_styles() {
    let sidebar = Sidebar::from_toml("modules = [\"pi-context\"]").unwrap();
    let context = starmux::PiContext {
        pull_requests: vec!["https://github.com/o/r/pull/1".into()],
        ..Default::default()
    };
    for (state, icon, style) in [
        (starmux::pr_state::PrState::Open, "\u{ea64}", "fg=green"),
        (
            starmux::pr_state::PrState::Draft,
            "\u{ebdb}",
            "fg=brightblack",
        ),
        (starmux::pr_state::PrState::Merged, "\u{eafe}", "fg=magenta"),
        (starmux::pr_state::PrState::Closed, "\u{ebda}", "fg=red"),
        (
            starmux::pr_state::PrState::Unknown,
            "\u{ea64}",
            "fg=brightblack",
        ),
    ] {
        let rendered = sidebar
            .render_with_inputs(
                &snapshot(),
                starmux::RenderInputs {
                    top: None,
                    pi_sessions: &[],
                    usage_rows: &[],
                    gob_jobs: &[],
                    context: Some(&context),
                    states: &[state],
                    commands: &BTreeMap::new(),
                    git: None,
                    debug: None,
                },
            )
            .unwrap();
        assert!(
            rendered.contains(&format!("#[{style}]{icon}")),
            "{rendered}"
        );
    }
}

#[test]
fn pi_context_icons_and_clipping_work_at_narrow_widths() {
    let sidebar = Sidebar::from_toml("modules = [\"pi-context\"]").unwrap();
    let dir = std::env::temp_dir().join(format!("starmux-context-rows-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let plan = dir.join("plan.md");
    let skill = dir.join("SKILL.md");
    std::fs::write(&plan, "# Plan").unwrap();
    std::fs::write(&skill, "# Skill").unwrap();
    let context = starmux::PiContext {
        session_id: "session-a".into(),
        plans: vec![starmux::PiPlan {
            title: "#[fg=red] Build an extensive search".into(),
            path: plan,
        }],
        pull_requests: vec!["https://github.com/owner/repo/pull/42".into()],
        skills: vec![starmux::PiSkill {
            name: "testing".into(),
            path: Some(skill),
        }],
    };
    for width in [16, 24, 30] {
        let mut input = snapshot();
        input.width = width;
        let rendered = sidebar
            .render_with_inputs(
                &input,
                starmux::RenderInputs {
                    top: None,
                    pi_sessions: &[],
                    usage_rows: &[],
                    gob_jobs: &[],
                    context: Some(&context),
                    states: &[starmux::pr_state::PrState::Draft],
                    commands: &BTreeMap::new(),
                    git: None,
                    debug: None,
                },
            )
            .unwrap();
        assert!(
            rendered.contains("◇") && rendered.contains("\u{ebdb}") && rendered.contains("✦"),
            "{rendered}"
        );
        assert!(rendered.contains("##[fg=red]"), "{rendered}");
        for kind in ["sl", "sr", "ss"] {
            assert_eq!(
                rendered.matches(&format!("#[range=user|{kind}")).count(),
                1,
                "{rendered}"
            );
        }
        assert_eq!(rendered.matches("#[range=").count(), 7, "{rendered}");
        for row in rendered.split("#[nl]").filter(|row| {
            ["sl", "sr", "ss"]
                .iter()
                .any(|kind| row.contains(&format!("#[range=user|{kind}")))
        }) {
            let token = row
                .split("#[range=user|")
                .nth(1)
                .unwrap()
                .split(' ')
                .next()
                .unwrap();
            assert_eq!(token.len(), 14);
        }
        assert_eq!(rendered.matches("#[nl]").count(), 9);
        assert!(rendered.contains(" π"), "{rendered}");
        if width == 30 {
            assert!(rendered.contains("owner/repo##42 draft"), "{rendered}");
        }
    }
    let long = starmux::PiContext {
        pull_requests: vec!["https://github.com/verylongowner/verylongrepository/pull/4242".into()],
        ..Default::default()
    };
    let mut input = snapshot();
    input.width = 16;
    let rendered = sidebar
        .render_with_inputs(
            &input,
            starmux::RenderInputs {
                top: None,
                pi_sessions: &[],
                usage_rows: &[],
                gob_jobs: &[],
                context: Some(&long),
                states: &[],
                commands: &BTreeMap::new(),
                git: None,
                debug: None,
            },
        )
        .unwrap();
    assert!(rendered.contains("…##4242"), "{rendered}");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn named_commands_keep_order_and_render_only_validated_styles() {
    let sidebar = Sidebar::from_toml(
        "modules = [\"command.gitmux\", \"divider\", \"command.other\"]\n\
         [commands.gitmux]\nargv = [\"gitmux\"]\noutput = \"tmux-styles\"\nprefix = \" \"\n\
         [commands.other]\nargv = [\"status\"]\nstyle = \"fg=green\"\n",
    )
    .unwrap();
    let commands = BTreeMap::from([
        (
            "command.gitmux".into(),
            "#[none]#[fg=white,bold]⎇ main #[range=user|bad]#{pane_id} ##[fg=red]".into(),
        ),
        ("command.other".into(), "second #[fg=red]#{pane_id}".into()),
    ]);
    let mut input = snapshot();
    input.width = 100;
    let rendered = sidebar
        .render_with_inputs(
            &input,
            starmux::RenderInputs {
                top: None,
                pi_sessions: &[],
                usage_rows: &[],
                gob_jobs: &[],
                context: None,
                states: &[],
                commands: &commands,
                git: None,
                debug: None,
            },
        )
        .unwrap();
    assert!(
        rendered.contains("#[default] #[fg=white,bold]⎇ main"),
        "{rendered}"
    );
    assert!(
        rendered.contains("##[range=user|bad]##{pane_id} ####[fg=red]"),
        "{rendered}"
    );
    assert!(
        rendered.contains("#[fg=green]second ##[fg=red]##{pane_id}"),
        "{rendered}"
    );
    assert!(rendered.find("⎇ main").unwrap() < rendered.find("----------------").unwrap());
    assert!(rendered.find("----------------").unwrap() < rendered.find("second").unwrap());
    assert_eq!(rendered.matches("##[range=user|bad]").count(), 1);
    let printed = sidebar.print_config().unwrap();
    assert!(printed.contains("[commands.gitmux]"));
    Sidebar::from_toml(&printed).unwrap();
}

#[test]
fn command_instances_require_exact_valid_references() {
    for config in [
        "modules = [\"command.gitmux\"]",
        "modules = [\"command.gitmux\", \"command.gitmux\"]\n[commands.gitmux]\nargv = [\"gitmux\"]",
        "modules = []\n[commands.gitmux]\nargv = [\"gitmux\"]",
        "modules = [\"command.bad.name\"]\n[commands.\"bad.name\"]\nargv = [\"gitmux\"]",
        "modules = [\"command.gitmux\"]\n[commands.gitmux]\nargv = []",
        "modules = [\"command.gitmux\"]\n[commands.gitmux]\nargv = [\"gitmux\"]\noutput = \"raw\"",
        "modules = [\"command.gitmux\"]\n[commands.gitmux]\nargv = [\"gitmux\"]\nstyle = \"range=user|bad\"",
    ] {
        assert!(Sidebar::from_toml(config).is_err(), "{config}");
    }
}

#[test]
fn git_lines_can_hide_upstream_or_put_it_on_its_own_row() {
    let status = starmux::GitStatus {
        branch: "main#[fg=red]".into(),
        upstream: "origin/main".into(),
        ahead: 2,
        added: 3,
        deleted: 1,
        ..Default::default()
    };
    let render = |lines: &str| {
        let sidebar = Sidebar::from_toml(&format!(
            "modules = [\"git\"]\n[git]\nlines = {lines}\nadded_style = \"fg=green\"\ndeleted_style = \"fg=red\""
        )).unwrap();
        sidebar
            .render_with_inputs(
                &snapshot(),
                starmux::RenderInputs {
                    top: None,
                    pi_sessions: &[],
                    usage_rows: &[],
                    gob_jobs: &[],
                    context: None,
                    states: &[],
                    commands: &BTreeMap::new(),
                    git: Some(&status),
                    debug: None,
                },
            )
            .unwrap()
    };
    let hidden = render("[\" $branch\", \" ( $added)( $deleted)\"]");
    assert!(!hidden.contains("origin/main"));
    assert!(hidden.contains("main##[fg=red]"), "{hidden}");
    assert!(hidden.contains("#[fg=green]+3"), "{hidden}");
    assert!(hidden.contains("#[fg=red]-1"), "{hidden}");
    let separate = render("[\" $branch\", \" $upstream( $divergence)\", \" ( $state)\"]");
    assert_eq!(separate.matches("#[nl]").count(), 4, "{separate}");
    assert!(separate.contains("origin/main"));
    assert!(separate.contains("↑2"));
    assert!(Sidebar::from_toml("modules = [\"git\"]\n[git]\nlines = [\"$unknown\"]").is_err());
}

#[test]
fn debug_rows_follow_order_and_clip_without_click_targets() {
    let sidebar = Sidebar::from_toml(
        "modules = [\"divider\", \"debug\"]\n[debug]\ndetails = true\nstyle = \"fg=brightblack\"",
    )
    .unwrap();
    let diagnostics = starmux::Diagnostics {
        stages: [1200, 0, 0, 250, 0, 0, 100, 50, 0],
    };
    let input = snapshot();
    let render = || {
        sidebar
            .render_with_inputs(
                &input,
                starmux::RenderInputs {
                    top: None,
                    pi_sessions: &[],
                    usage_rows: &[],
                    gob_jobs: &[],
                    context: None,
                    states: &[],
                    commands: &BTreeMap::new(),
                    git: None,
                    debug: Some(&diagnostics),
                },
            )
            .unwrap()
    };
    let rendered = render();
    assert!(rendered.find("----------------").unwrap() < rendered.find("last 1.60 ms").unwrap());
    assert!(rendered.contains("tmux 1.20 ms"), "{rendered}");
    assert!(rendered.contains("usage 0.250 ms"), "{rendered}");
    assert!(rendered.contains("git 0.100 ms"), "{rendered}");
    assert!(rendered.contains("#[range=user|sv "), "{rendered}");
    assert!(!rendered.contains("#[range=user|sw"), "{rendered}");
    assert!(rendered.contains("#[fg=brightblack]"), "{rendered}");
    let mut narrow = input.clone();
    narrow.width = 7;
    let clipped = sidebar
        .render_with_inputs(
            &narrow,
            starmux::RenderInputs {
                top: None,
                pi_sessions: &[],
                usage_rows: &[],
                gob_jobs: &[],
                context: None,
                states: &[],
                commands: &BTreeMap::new(),
                git: None,
                debug: Some(&diagnostics),
            },
        )
        .unwrap();
    assert!(!clipped.contains("last 1.60 ms"), "{clipped}");
    assert!(clipped.contains("#[range=user|sv "), "{clipped}");

    let disabled = Sidebar::from_toml("modules = [\"debug\"]\n[debug]\ndisabled = true").unwrap();
    assert!(!disabled.render(&input).unwrap().contains("last"));
    assert!(
        Sidebar::from_toml("modules = [\"debug\"]\n[debug]\ncache_dir = \"relative\"").is_err()
    );
    assert!(Sidebar::from_toml("modules = [\"debug\"]\n[debug]\nstyle = \"#[bad]\"").is_err());
}

#[test]
fn bundled_schemes_color_all_rendered_modules_and_round_trip() {
    let config = r#"
colorscheme = "tokyo-night"
modules = ["sessions", "divider", "pi-workbench", "pi-context", "usage", "gob", "git", "debug", "command.example"]
[commands.example]
argv = ["true"]
[usage]
providers = ["codex"]
"#;
    let sidebar = Sidebar::from_toml(config).unwrap();
    let pi = PiSession {
        state: "working".into(),
        project: "/tmp/project".into(),
        name: "agent".into(),
        location: None,
        target: None,
        selected: true,
    };
    let context = starmux::PiContext {
        pull_requests: vec!["https://github.com/o/r/pull/1".into()],
        ..Default::default()
    };
    let usage = UsageRow {
        provider: "codex".into(),
        display_name: "Codex".into(),
        available_resets: None,
        windows: vec![UsageWindow {
            label: "5h".into(),
            used_percent: 90.0,
            duration_seconds: None,
            reset_at: None,
        }],
        stale: false,
        unavailable: false,
        refresh_failure: None,
        fetched_at: None,
    };
    let job = starmux::GobJob {
        id: "1".into(),
        name: "build".into(),
        started_at: None,
        avg_duration_ms: 0,
    };
    let git = starmux::GitStatus {
        branch: "main".into(),
        modified: 2,
        ..Default::default()
    };
    let debug = starmux::Diagnostics {
        stages: [1000, 0, 0, 0, 0, 0, 0, 0, 0],
    };
    let commands = BTreeMap::from([("command.example".into(), "hello".into())]);
    let render = |sidebar: &Sidebar| {
        sidebar
            .render_with_inputs(
                &snapshot(),
                starmux::RenderInputs {
                    top: None,
                    pi_sessions: std::slice::from_ref(&pi),
                    usage_rows: std::slice::from_ref(&usage),
                    gob_jobs: std::slice::from_ref(&job),
                    context: Some(&context),
                    states: &[starmux::pr_state::PrState::Open],
                    commands: &commands,
                    git: Some(&git),
                    debug: Some(&debug),
                },
            )
            .unwrap()
    };
    let output = render(&sidebar);
    for expected in [
        "#[fg=#1b1d2b,bg=#c099ff,bold]", // sessions
        "#[fg=#3b4261,nobold]",          // divider
        "fg=#ffc777,bg=#3b4261]●",       // selected Pi Workbench
        "#[fg=#c3e88d]",                 // Pi context
        "#[fg=#f7768e",                  // usage and git
        "#[fg=#82aaff,bold] Jobs",       // gob
        "#[fg=#828bb8] last",            // debug
        "#[fg=#82aaff]hello",            // command
    ] {
        assert!(output.contains(expected), "missing {expected}: {output}");
    }
    let printed = sidebar.print_config().unwrap();
    assert_eq!(output, render(&Sidebar::from_toml(&printed).unwrap()));

    for (name, color) in [
        ("catppuccin-mocha", "#cba6f7"),
        ("github-dark", "#a371f7"),
        ("gruvbox-dark", "#d3869b"),
        ("nord", "#b48ead"),
        ("dracula", "#bd93f9"),
        ("solarized-dark", "#6c71c4"),
        ("one-dark", "#c678dd"),
        ("rose-pine-moon", "#c4a7e7"),
        ("kanagawa-wave", "#957fb8"),
    ] {
        let themed = Sidebar::from_toml(&format!("colorscheme = \"{name}\"")).unwrap();
        assert!(themed
            .render(&snapshot())
            .unwrap()
            .contains(&format!("bg={color}")));
    }
}

#[test]
fn scheme_overrides_and_invalid_inputs_are_checked_at_the_sidebar_interface() {
    let sidebar = Sidebar::from_toml(
        r##"
colorscheme = "tokyo-night"
palette = "mine"
[palettes.mine]
accent = "#abcdef"
[sessions]
other_session_style = "fg=red,bold"
current_session_fill = "#123456"
"##,
    )
    .unwrap();
    let output = sidebar.render(&snapshot()).unwrap();
    assert!(output.contains("bg=#abcdef"), "{output}");
    assert!(output.contains("#[fill=#123456]"), "{output}");
    assert!(output.contains("#[fg=red,bold]"), "{output}");
    assert!(Sidebar::from_toml("colorscheme = \"missing\"").is_err());
    assert!(Sidebar::from_toml(
        "colorscheme = \"tokyo-night\"\n[usage]\nbar_track_color = \"#[bad]\""
    )
    .is_err());
    assert_eq!(
        Sidebar::defaults().unwrap().render(&snapshot()).unwrap(),
        Sidebar::from_toml("modules = [\"sessions\", \"divider\"]")
            .unwrap()
            .render(&snapshot())
            .unwrap()
    );
}

#[test]
fn custom_colorscheme_colors_modules_and_survives_print_config() {
    let config = r##"
colorscheme = "my-dark"
modules = ["sessions", "divider", "command.status"]
[colorschemes.my-dark]
background = "#10151c"
surface = "#18212b"
highlight = "#273442"
border = "#405064"
text = "#d9e2ec"
muted = "#91a2b3"
accent = "#a8a0ff"
warning = "#ffd580"
green = "#8fd6a8"
orange = "#ffab70"
danger = "#ff808c"
[commands.status]
argv = ["status"]
"##;
    let sidebar = Sidebar::from_toml(config).unwrap();
    let commands = BTreeMap::from([("command.status".into(), "ready".into())]);
    let render = |sidebar: &Sidebar| {
        sidebar
            .render_with_inputs(
                &snapshot(),
                starmux::RenderInputs {
                    top: None,
                    pi_sessions: &[],
                    usage_rows: &[],
                    gob_jobs: &[],
                    context: None,
                    states: &[],
                    commands: &commands,
                    git: None,
                    debug: None,
                },
            )
            .unwrap()
    };
    let output = render(&sidebar);
    assert!(output.contains("#[fg=#10151c,bg=#a8a0ff,bold]"), "{output}");
    assert!(output.contains("#[fg=#405064,nobold]"), "{output}");
    assert!(output.contains("#[fg=#d9e2ec]ready"), "{output}");
    let printed = sidebar.print_config().unwrap();
    assert!(printed.contains("[colorschemes.my-dark]"));
    assert_eq!(output, render(&Sidebar::from_toml(&printed).unwrap()));

    for broken in [
        config.replace("danger = \"#ff808c\"", ""),
        config.replace("danger = \"#ff808c\"", "danger = \"fg=red\""),
        config.replace("danger = \"#ff808c\"", "surprise = \"#ff808c\""),
        config.replace("my-dark", "github-dark"),
    ] {
        assert!(Sidebar::from_toml(&broken).is_err(), "accepted {broken}");
    }
}

#[test]
fn named_configs_select_rows_with_shared_settings_and_round_trip() {
    let text = r#"
modules = ["sessions", "divider"]
[configs.right]
modules = ["divider", "command.build", "blank"]
[configs.empty]
modules = []
[divider]
character = "="
[commands.build]
argv = ["echo", "ready"]
"#;
    let default = Sidebar::from_toml(text).unwrap();
    let printed = default.print_config().unwrap();
    assert!(printed.contains("[configs.right]"));
    let right = Sidebar::from_toml(&printed)
        .unwrap()
        .select("right")
        .unwrap();
    assert!(default
        .render(&snapshot())
        .unwrap()
        .contains("#[range=user|st0 "));
    let rendered = right.render(&snapshot()).unwrap();
    assert!(
        rendered.contains("============================"),
        "{rendered}"
    );
    assert!(!rendered.contains("#[range=user|st0 "), "{rendered}");
    assert!(Sidebar::from_toml(text)
        .unwrap()
        .select("empty")
        .unwrap()
        .render(&snapshot())
        .is_ok());
    assert!(Sidebar::from_toml(text).unwrap().select("missing").is_err());
}

#[test]
fn slim_selection_hides_windows_and_has_independent_module_order() {
    let text = "modules = ['sessions', 'divider']\n[slim]\nmodules = ['divider', 'sessions']";
    let sidebar = Sidebar::from_toml(text).unwrap();
    let mut unsupported = snapshot();
    unsupported.width = 1;
    assert!(sidebar
        .render(&unsupported)
        .unwrap_err()
        .contains("width must be 2..300"));
    for width in [2, 3, 30] {
        let mut input = snapshot();
        input.width = width;
        let output = sidebar.render(&input).unwrap();
        let rows: Vec<_> = output.split("#[nl]").skip(2).collect();
        if width <= 2 {
            assert!(rows[0].contains(&"-".repeat(width)));
            assert!(rows[1].contains("range=user|st0 list=focus"));
            assert!(!output.contains("range=user|sw"));
            assert_eq!(rows.len(), 4);
        } else {
            assert!(rows[0].contains("range=user|st0"));
            assert!(output.contains("range=user|sw0 list=focus"));
        }
    }
    let mut input = snapshot();
    input.width = 2;
    let full = Sidebar::from_toml("[slim]\nmode = 'full'")
        .unwrap()
        .render(&input)
        .unwrap();
    assert!(full.contains("range=user|sw0"));
    let shown = Sidebar::from_toml("[sessions]\nshow_windows = false\n[slim]\nshow_windows = true")
        .unwrap();
    assert!(shown
        .render(&input)
        .unwrap()
        .contains("range=user|sw0 list=focus"));
    input.width = 30;
    assert!(!shown.render(&input).unwrap().contains("range=user|sw"));
    let forced = Sidebar::from_toml("modules = ['sessions']\n[slim]\nmode = 'slim'").unwrap();
    let output = forced.render(&input).unwrap();
    assert!(!output.contains("main"));
    assert!(output.contains("󰆍●"));
    assert!(!output.contains("range=user|sw"));
}

#[test]
fn slim_icons_are_independent_of_untrusted_names() {
    let mut input = snapshot();
    input.width = 2;
    input.sessions[0].name = "界🦀".into();
    input.sessions[1].name = "\u{301}\n #[range=user|evil]".into();
    let sidebar = Sidebar::defaults().unwrap();
    let output = sidebar.render(&input).unwrap();
    assert_eq!(output.matches("󰆍").count(), 2, "{output}");
    assert!(!output.contains("##"), "{output}");
    assert!(!output.contains("range=user|evil"));
    assert!(!output.contains("界"), "{output}");
    let empty = Sidebar::from_toml("[slim]\nmodules = []").unwrap();
    assert!(!empty.render(&input).unwrap().contains("range=user|"));
}

#[test]
fn slim_configuration_validates_every_list_and_command_glyph() {
    for text in [
        "[slim]\nmode = 'compact'",
        "[slim]\nmodules = ['unknown']",
        "[slim]\nmodules = ['sessions', 'sessions']",
        "[slim]\nmodules = ['spacer', 'spacer']",
        "[slim]\nmodules = ['command.missing']",
        "[configs.right]\nmodules = []\nslim_modules = ['missing']",
        "[configs.right]\nmodules = []\nslim_modules = ['sessions', 'sessions']",
        "[slim]\nunknown = true",
    ] {
        assert!(Sidebar::from_toml(text).is_err(), "accepted {text}");
    }
    for glyph in ["", " ", "ab", "界", "\n", "\u{301}"] {
        let text = format!(
            "modules = ['command.test']\n[commands.test]\nargv = ['echo']\nslim_icon = {glyph:?}"
        );
        assert!(Sidebar::from_toml(&text).is_err(), "accepted {glyph:?}");
    }
    let text = "modules = []\n[configs.right]\nmodules = []\nslim_modules = ['command.test']\n[commands.test]\nargv = ['echo']\nslim_icon = '#'";
    let sidebar = Sidebar::from_toml(text).unwrap();
    let roundtrip = Sidebar::from_toml(&sidebar.print_config().unwrap())
        .unwrap()
        .select("right")
        .unwrap();
    let mut input = snapshot();
    input.width = 2;
    let commands = BTreeMap::from([("command.test".into(), "data".into())]);
    let output = roundtrip
        .render_with_inputs(
            &input,
            starmux::RenderInputs {
                top: None,
                pi_sessions: &[],
                usage_rows: &[],
                gob_jobs: &[],
                context: None,
                states: &[],
                commands: &commands,
                git: None,
                debug: None,
            },
        )
        .unwrap();
    assert!(output.contains("## #[norange"), "{output}");
}

#[test]
fn check_config_validates_all_named_lists() {
    for text in [
        "modules = []\n[configs.right]\nmodules = [\"missing\"]",
        "modules = []\n[configs.right]\nmodules = [\"sessions\", \"sessions\"]",
        "modules = []\n[configs.right]\nmodules = [\"spacer\", \"spacer\"]",
        "modules = []\n[configs.default]\nmodules = []",
        "modules = []\n[configs.right]\nmodules = []\nunknown = 1",
        "modules = []\n[configs.right]\nmodules = [\"command.missing\"]",
    ] {
        assert!(Sidebar::from_toml(text).is_err(), "accepted {text}");
    }
}
