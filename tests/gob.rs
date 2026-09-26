use starmux::{GobJob, Session, Sidebar, Snapshot};
use time::OffsetDateTime;

fn snapshot(width: usize) -> Snapshot {
    Snapshot {
        width,
        client_width: 100,
        client_height: 30,
        current_session: "$0".into(),
        current_pane: "%0".into(),
        pane_path: "/tmp".into(),
        sessions: vec![Session {
            id: "$0".into(),
            name: "main".into(),
            windows: vec![],
        }],
    }
}

#[test]
fn gob_jobs_show_running_dots_and_plain_progress_after_the_divider() {
    let sidebar = Sidebar::from_toml("modules = [\"divider\", \"gob\"]").unwrap();
    let jobs = [
        GobJob {
            id: "a".into(),
            name: "build #[range=user|evil]#{pane_id}".into(),
            started_at: Some(OffsetDateTime::now_utc() - time::Duration::seconds(5)),
            avg_duration_ms: 10_000,
        },
        GobJob {
            id: "b".into(),
            name: "dev server".into(),
            started_at: None,
            avg_duration_ms: 0,
        },
    ];
    let rendered = sidebar
        .render_with_modules(&snapshot(40), &[], &[], &jobs)
        .unwrap();
    assert!(rendered.find("------").unwrap() < rendered.find(" Jobs").unwrap());
    assert!(
        rendered.contains("#[fg=green]●#[default] build ##[range=user|evil]##{pane_id}"),
        "{rendered}"
    );
    assert!(rendered.contains("#[fg=green,bg=colour238]"), "{rendered}");
    assert!(rendered.contains("50%"), "{rendered}");
    assert!(!rendered.contains("est ") && !rendered.contains("#[range=user|evil]#{pane_id}"));
    assert_eq!(rendered.matches("●").count(), 2);
    assert_eq!(rendered.matches("%#[nl]").count(), 1);
    assert!(rendered
        .split("#[nl]")
        .skip(2)
        .all(|row| !row.starts_with("#[range=")));
}

#[test]
fn narrow_gob_rows_keep_the_percentage_and_empty_jobs_add_no_heading() {
    let sidebar = Sidebar::from_toml("modules = [\"gob\"]").unwrap();
    let job = GobJob {
        id: "x".into(),
        name: "long running job".into(),
        started_at: Some(OffsetDateTime::now_utc() - time::Duration::seconds(30)),
        avg_duration_ms: 10_000,
    };
    let rendered = sidebar
        .render_with_modules(&snapshot(8), &[], &[], &[job])
        .unwrap();
    assert!(rendered.contains("100%"), "{rendered}");
    assert!(!rendered.contains("█"), "{rendered}");
    let empty = sidebar
        .render_with_modules(&snapshot(8), &[], &[], &[])
        .unwrap();
    assert!(!empty.contains("Jobs"));
}

#[test]
fn gob_is_optional_and_its_styles_and_variables_are_validated() {
    let printed = Sidebar::defaults().unwrap().print_config().unwrap();
    assert!(printed.contains("[gob]"));
    assert!(!Sidebar::defaults()
        .unwrap()
        .render(&snapshot(30))
        .unwrap()
        .contains("Jobs"));
    for text in [
        "modules = [\"gob\", \"gob\"]",
        "[gob]\nformat = \"$cwd\"",
        "[gob]\nrunning_style = \"fg=#[bad]\"",
        "[gob]\nbar_track_color = \"invalid\"",
    ] {
        assert!(Sidebar::from_toml(text).is_err(), "accepted {text}");
    }
}
