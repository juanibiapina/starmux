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

fn job(elapsed_ms: i64, typical_ms: u64, upper_ms: u64) -> GobJob {
    GobJob {
        id: "x".into(),
        name: "build".into(),
        started_at: Some(OffsetDateTime::now_utc() - time::Duration::milliseconds(elapsed_ms)),
        expected_duration_ms: typical_ms,
        expected_upper_duration_ms: upper_ms,
    }
}

fn progress_row(sidebar: &Sidebar, width: usize, job: &GobJob) -> (String, String) {
    let rendered = sidebar
        .render_with_modules(&snapshot(width), &[], &[], std::slice::from_ref(job))
        .unwrap();
    let row = rendered.split("#[nl]").nth(4).unwrap().to_owned();
    (visible_row(&row), row)
}

#[test]
fn gob_jobs_show_running_dots_and_progress_only_with_an_estimate() {
    let sidebar = Sidebar::from_toml("modules = [\"divider\", \"gob\"]").unwrap();
    let jobs = [
        GobJob {
            name: "build #[range=user|evil]#{pane_id}".into(),
            ..job(5_000, 10_000, 10_000)
        },
        GobJob {
            id: "b".into(),
            name: "dev server".into(),
            ..job(5_000, 0, 0)
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
    let progress = visible_row(rendered.split("#[nl]").nth(5).unwrap());
    assert_eq!(progress.chars().count(), 40, "{rendered}");
    assert!(progress.starts_with("    █"), "{rendered}");
    assert!(progress.ends_with("   5s / ~10s  "), "{rendered}");
    assert!(!rendered.contains("fg=yellow"), "{rendered}");
    assert!(rendered.contains("#[dim]5s / ~10s"), "{rendered}");
    assert!(!rendered.contains("#[range=user|evil]#{pane_id}"));
    assert_eq!(rendered.matches("●").count(), 2);
    assert_eq!(rendered.matches("#[nl]").count(), 7);
    assert!(rendered
        .split("#[nl]")
        .skip(2)
        .filter(|row| !row.is_empty())
        .all(|row| row.contains("#[range=user|sv ") && !row.contains("#[range=user|sw")));
}

#[test]
fn gob_progress_fills_past_the_typical_duration_in_overdue_style() {
    let sidebar = Sidebar::from_toml("modules = [\"gob\"]").unwrap();
    let (visible, row) = progress_row(&sidebar, 40, &job(64_000, 20_000, 140_000));
    assert_eq!(visible, "    █████████▏             1m4s / ~20s  ", "{row}");
    assert!(
        row.contains(
            "#[fg=green,bg=colour238]███#[fg=yellow,bg=colour238]██████▏#[bg=colour238]          "
        ),
        "{row}"
    );
    assert!(row.contains("#[dim]1m4s / ~20s"), "{row}");
}

#[test]
fn gob_progress_past_the_upper_bound_is_full_with_an_overdue_label() {
    let sidebar = Sidebar::from_toml("modules = [\"gob\"]").unwrap();
    let (visible, row) = progress_row(&sidebar, 40, &job(2_090_000, 1_000_000, 2_000_000));
    assert_eq!(visible, "    █████████████████ 34m50s / ~16m40s  ", "{row}");
    assert!(
        row.contains("#[fg=green,bg=colour238]█████████#[fg=yellow,bg=colour238]████████"),
        "{row}"
    );
    assert!(row.contains("#[fg=yellow]34m50s / ~16m40s"), "{row}");

    let custom = Sidebar::from_toml(
        "modules = [\"gob\"]\n[gob]\noverdue_style = \"fg=red\"\nlabel_style = \"fg=blue\"",
    )
    .unwrap();
    let (_, row) = progress_row(&custom, 40, &job(2_090_000, 1_000_000, 2_000_000));
    assert!(row.contains("#[fg=red]34m50s"), "{row}");
    let (_, row) = progress_row(&custom, 40, &job(64_000, 20_000, 140_000));
    assert!(row.contains("#[fg=blue]1m4s"), "{row}");
}

#[test]
fn gob_label_formats_durations_and_keeps_the_bar_length_within_a_magnitude() {
    let sidebar = Sidebar::from_toml("modules = [\"gob\"]").unwrap();
    let label = |elapsed_ms| progress_row(&sidebar, 40, &job(elapsed_ms, 7_200_000, 7_200_000)).0;
    for (elapsed_ms, expected) in [
        (500, "<1s / ~2h  "),
        (59_000, "59s / ~2h  "),
        (64_000, "1m4s / ~2h  "),
        (120_000, "2m / ~2h  "),
        (3_900_000, "1h5m / ~2h  "),
    ] {
        assert!(
            label(elapsed_ms).ends_with(expected),
            "{}",
            label(elapsed_ms)
        );
    }
    let slash = |elapsed_ms| label(elapsed_ms).chars().position(|c| c == '/');
    assert_eq!(slash(10_000), slash(59_000));
    assert_eq!(slash(64_000), slash(120_000));
}

#[test]
fn gob_track_resizes_and_starts_empty() {
    let sidebar = Sidebar::from_toml("modules = [\"gob\"]").unwrap();
    let overdue = job(29_500, 10_000, 10_000);
    for (width, expected) in [
        (40, "    ███████████████████████ 29s / ~10s  "),
        (12, "    ██ 29s  "),
        (8, " █ 29s  "),
        (5, "29s  "),
        (3, "29s"),
    ] {
        assert_eq!(progress_row(&sidebar, width, &overdue).0, expected);
    }

    let waiting = job(-10_000, 10_000, 10_000);
    let (visible, row) = progress_row(&sidebar, 8, &waiting);
    assert_eq!(visible, "   <1s  ");
    assert!(row.contains("#[bg=colour238]"));

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
        "[gob]\noverdue_style = \"fg=#[bad]\"",
        "[gob]\nlabel_style = \"fg=#[bad]\"",
    ] {
        assert!(Sidebar::from_toml(text).is_err(), "accepted {text}");
    }
}

#[test]
fn slim_gob_gauge_fills_to_typical_and_turns_overdue_past_it() {
    let sidebar = Sidebar::from_toml("modules = [\"gob\"]").unwrap();
    let gauge = |job: GobJob| {
        let rendered = sidebar
            .render_with_modules(&snapshot(2), &[], &[], &[job])
            .unwrap();
        rendered
            .split("#[nl]")
            .find(|row| row.contains('↳'))
            .unwrap()
            .to_owned()
    };
    assert!(gauge(job(5_000, 10_000, 20_000)).contains("#[fg=green]↳▄"));
    assert!(gauge(job(15_000, 10_000, 20_000)).contains("#[fg=yellow]↳█"));
    assert!(gauge(job(25_000, 10_000, 20_000)).contains("#[fg=yellow]↳█"));
}
