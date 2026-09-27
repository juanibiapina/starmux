use starmux::{GobJob, Session, Sidebar, Snapshot};
use time::OffsetDateTime;

fn snapshot(width: usize) -> Snapshot {
    Snapshot {
        width,
        client_width: 100,
        client_height: 30,
        status_lines: 1,
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

fn visible_row(row: &str) -> String {
    row.split("#[")
        .map(|part| part.split_once(']').map_or(part, |(_, text)| text))
        .collect()
}

#[test]
fn gob_jobs_show_running_dots_and_horizontal_progress_after_the_divider() {
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
    let progress = visible_row(rendered.split("#[nl]").nth(5).unwrap());
    assert_eq!(progress.chars().count(), 40, "{rendered}");
    assert!(progress.starts_with("    █"), "{rendered}");
    assert!(progress.ends_with("  50%  "), "{rendered}");
    assert!(!progress.contains(['░', '▁']), "{rendered}");
    let completed = GobJob {
        started_at: Some(OffsetDateTime::now_utc() - time::Duration::seconds(30)),
        ..jobs[0].clone()
    };
    let full = sidebar
        .render_with_modules(&snapshot(40), &[], &[], &[completed])
        .unwrap();
    let full_progress = visible_row(full.split("#[nl]").nth(5).unwrap());
    assert!(full_progress.ends_with(" 100%  "), "{full}");
    assert_eq!(
        progress.chars().position(|character| character == '%'),
        full_progress.chars().position(|character| character == '%')
    );
    assert_eq!(progress.chars().count(), full_progress.chars().count());
    assert!(!rendered.contains("est ") && !rendered.contains("#[range=user|evil]#{pane_id}"));
    assert_eq!(rendered.matches("●").count(), 2);
    assert_eq!(rendered.matches("#[nl]").count(), 7);
    assert!(rendered
        .split("#[nl]")
        .skip(2)
        .filter(|row| !row.is_empty())
        .all(|row| row.contains("#[range=user|sv ") && !row.contains("#[range=user|sw")));
}

#[test]
fn gob_track_resizes_and_shows_zero_and_capped_progress() {
    let sidebar = Sidebar::from_toml("modules = [\"gob\"]").unwrap();
    let job = GobJob {
        id: "x".into(),
        name: "long running job".into(),
        started_at: Some(OffsetDateTime::now_utc() - time::Duration::seconds(30)),
        avg_duration_ms: 10_000,
    };
    for (width, expected) in [
        (12, "    █ 100%  "),
        (8, "█ 100%  "),
        (5, "█100%"),
        (3, "100"),
    ] {
        let rendered = sidebar
            .render_with_modules(&snapshot(width), &[], &[], std::slice::from_ref(&job))
            .unwrap();
        assert_eq!(
            visible_row(rendered.split("#[nl]").nth(4).unwrap()),
            expected
        );
    }

    let waiting = GobJob {
        started_at: Some(OffsetDateTime::now_utc() + time::Duration::seconds(10)),
        ..job
    };
    let rendered = sidebar
        .render_with_modules(&snapshot(8), &[], &[], &[waiting])
        .unwrap();
    assert_eq!(
        visible_row(rendered.split("#[nl]").nth(4).unwrap()),
        "    0%  "
    );
    assert!(rendered.contains("#[bg=colour238]"));

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
