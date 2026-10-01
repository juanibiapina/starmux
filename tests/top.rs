use starmux::{
    top::{Battery, HostStatus},
    RenderInputs, Session, Sidebar, Snapshot, Window,
};
use std::collections::BTreeMap;
use unicode_width::UnicodeWidthStr;

fn visible_row(row: &str) -> String {
    row.split("#[")
        .filter_map(|part| part.split_once(']').map(|(_, text)| text))
        .collect()
}

fn metric_row(output: &str, metric: &str) -> String {
    visible_row(
        output
            .split("#[nl]")
            .find(|row| row.contains(metric))
            .unwrap(),
    )
}

fn bar_column(row: &str) -> usize {
    UnicodeWidthStr::width(row.split(['▰', '▱']).next().unwrap())
}

fn snapshot(width: usize) -> Snapshot {
    Snapshot {
        width,
        client_width: 80,
        client_height: 30,
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
                name: "shell".into(),
                selected: true,
                pane: "%0".into(),
                path: "/tmp".into(),
                options: BTreeMap::new(),
            }],
        }],
    }
}

fn render(sidebar: &Sidebar, status: Option<&HostStatus>, width: usize) -> String {
    sidebar
        .render_with_inputs(
            &snapshot(width),
            RenderInputs {
                top: status,
                pi_sessions: &[],
                usage_rows: &[],
                gob_jobs: &[],
                context: None,
                states: &[],
                commands: &BTreeMap::new(),
                git: None,
                debug: None,
            },
        )
        .unwrap()
}

#[test]
fn slim_host_pies_show_utilization_and_remaining_charge() {
    let sidebar = Sidebar::from_toml("modules = ['top']\ncolorscheme = 'tokyo-night'").unwrap();
    for (cpu, memory, battery, expected) in [
        (25, 50, 75, ["󰻠◔", "󰍛◑", "󰁹◕"]),
        (90, 80, 10, ["󰻠●", "󰍛◕", "󰁺○"]),
    ] {
        let status = HostStatus {
            cpu: Some(cpu),
            memory: Some(memory),
            battery: Some(Battery {
                percent: battery,
                charging: false,
                full: false,
            }),
            age_seconds: None,
        };
        let output = render(&sidebar, Some(&status), 2);
        let rows: Vec<_> = output
            .split("#[nl]")
            .skip(2)
            .map(|row| visible_row(row).trim().to_owned())
            .filter(|row| !row.is_empty())
            .collect();
        assert_eq!(rows, expected);
        assert!(rows
            .iter()
            .all(|row| UnicodeWidthStr::width(row.as_str()) == 2));
        if cpu >= 80 {
            assert!(
                output.contains("fg=#f7768e") && output.contains("bold"),
                "{output}"
            );
        }
    }
    let mut status = HostStatus {
        cpu: None,
        memory: Some(0),
        battery: Some(Battery {
            percent: 50,
            charging: true,
            full: false,
        }),
        age_seconds: None,
    };
    let rows = |output: String| -> Vec<String> {
        output
            .split("#[nl]")
            .skip(2)
            .map(|row| visible_row(row).trim().to_owned())
            .filter(|row| !row.is_empty())
            .collect()
    };
    assert_eq!(rows(render(&sidebar, Some(&status), 2)), ["󰻠󰋗", "󰍛○", "󰂄◑"]);
    status.battery = Some(Battery {
        percent: 100,
        charging: true,
        full: true,
    });
    status.age_seconds = Some(5);
    assert_eq!(rows(render(&sidebar, Some(&status), 2)), ["󰻠󰋗", "󰍛○", "󱟢●"]);
    status.battery = None;
    assert_eq!(rows(render(&sidebar, Some(&status), 2)), ["󰻠󰋗", "󰍛○"]);
}

#[test]
fn top_shows_host_metrics_in_configured_order_and_hides_absent_battery() {
    let sidebar = Sidebar::from_toml("modules = [\"top\"]\ncolorscheme = \"tokyo-night\"\n[top]\nmetrics = [\"memory\", \"cpu\", \"battery\"]").unwrap();
    let status = HostStatus {
        cpu: Some(81),
        memory: Some(36),
        battery: None,
        age_seconds: None,
    };
    let output = render(&sidebar, Some(&status), 30);
    assert!(
        output.find("MEM").unwrap() < output.find("CPU").unwrap(),
        "{output}"
    );
    assert!(
        output.contains("81%") && output.contains("36%") && !output.contains("BAT"),
        "{output}"
    );
    assert!(output.contains("fg=#f7768e"), "{output}");
    assert!(!output.contains("#[range=user|su"), "{output}");
    let printed = sidebar.print_config().unwrap();
    assert_eq!(
        output,
        render(&Sidebar::from_toml(&printed).unwrap(), Some(&status), 30)
    );
}

#[test]
fn top_bars_align_across_metrics_and_percentage_changes() {
    let sidebar = Sidebar::from_toml("modules = [\"top\"]").unwrap();
    for (cpu, memory, battery) in [(9, 42, 100), (100, 9, 42), (42, 100, 9)] {
        let status = HostStatus {
            cpu: Some(cpu),
            memory: Some(memory),
            battery: Some(Battery {
                percent: battery,
                charging: false,
                full: false,
            }),
            age_seconds: None,
        };
        let output = render(&sidebar, Some(&status), 36);
        let rows = ["CPU", "MEM", "BAT"].map(|metric| metric_row(&output, metric));
        assert!(rows.iter().all(|row| bar_column(row) == 15), "{output}");
        let filled = rows[2].chars().filter(|&ch| ch == '▰').count();
        let empty = rows[2].chars().filter(|&ch| ch == '▱').count();
        assert_eq!(
            (filled, empty),
            match battery {
                9 => (1, 7),
                42 => (4, 4),
                100 => (8, 0),
                _ => unreachable!(),
            },
            "{output}"
        );
        assert!(rows[2].contains(&format!("{battery}%")), "{output}");
    }
}

#[test]
fn top_shows_battery_and_keeps_values_when_the_sidebar_is_narrow() {
    let sidebar = Sidebar::from_toml("modules = [\"top\"]").unwrap();
    let status = HostStatus {
        cpu: Some(12),
        memory: None,
        battery: Some(Battery {
            percent: 19,
            charging: true,
            full: false,
        }),
        age_seconds: Some(75),
    };
    let wide = render(&sidebar, Some(&status), 30);
    assert!(
        wide.contains("BAT") && wide.contains("19%") && wide.contains("󰂄"),
        "{wide}"
    );
    assert!(wide.contains("75s old") && wide.contains("--"), "{wide}");
    let battery = metric_row(&wide, "BAT");
    assert_eq!(
        battery.chars().filter(|&ch| ch == '▰' || ch == '▱').count(),
        8
    );
    assert_eq!(bar_column(&battery), 15);
    assert!(
        battery.contains("▰") && !battery.contains("charging"),
        "{wide}"
    );
    let narrow = render(&sidebar, Some(&status), 15);
    assert!(
        narrow.contains("19%") && !narrow.contains("charging"),
        "{narrow}"
    );
    assert!(!metric_row(&narrow, "BAT").contains('▰'));
    assert!(UnicodeWidthStr::width(metric_row(&narrow, "BAT").as_str()) <= 15);
    let compact = metric_row(&render(&sidebar, Some(&status), 16), "BAT");
    assert!(compact.contains("19%  ▰") && !compact.contains("charging"));
    assert!(UnicodeWidthStr::width(compact.as_str()) <= 16);
    assert!(render(&sidebar, Some(&status), 2).contains("#[nl]"));

    let fresh = HostStatus {
        age_seconds: None,
        ..status
    };
    let critical = render(&sidebar, Some(&fresh), 30);
    assert!(critical.contains("#[fg=red]  ▰"), "{critical}");
}

#[test]
fn top_battery_icons_show_state_without_shortening_the_bar() {
    let sidebar = Sidebar::from_toml("modules = [\"top\"]").unwrap();
    for (percent, charging, full, icon) in [
        (100, false, true, "󱟢"),
        (98, true, true, "󱟢"),
        (98, true, false, "󰂄"),
        (19, true, false, "󰂄"),
        (20, false, false, "󰁺"),
        (21, false, false, "󰁹"),
        (100, false, false, "󰁹"),
    ] {
        let status = HostStatus {
            cpu: Some(12),
            memory: Some(42),
            battery: Some(Battery {
                percent,
                charging,
                full,
            }),
            age_seconds: None,
        };
        for width in [15, 16, 30] {
            let output = render(&sidebar, Some(&status), width);
            let battery = metric_row(&output, "BAT");
            assert!(battery.starts_with(&format!(" {icon} BAT")), "{output}");
            assert!(battery.contains(&format!("{percent}%")), "{output}");
            assert!(
                !battery.contains("charging") && !battery.contains("full"),
                "{output}"
            );
            assert!(
                UnicodeWidthStr::width(battery.as_str()) <= width,
                "{output}"
            );
            let cells = battery.chars().filter(|&ch| ch == '▰' || ch == '▱').count();
            assert_eq!(
                cells,
                match width {
                    15 => 0,
                    16 => 1,
                    _ => 8,
                },
                "{output}"
            );
            if width == 30 {
                for metric in ["CPU", "MEM", "BAT"] {
                    assert_eq!(bar_column(&metric_row(&output, metric)), 15, "{output}");
                }
            }
        }
    }
    let unavailable = metric_row(&render(&sidebar, None, 30), "BAT");
    assert!(unavailable.starts_with(" 󰁹 BAT") && unavailable.contains("--"));
}

#[test]
fn top_and_shared_cache_configuration_reject_invalid_values() {
    for config in [
        "modules = [\"top\"]\n[top]\nmetrics = [\"cpu\", \"cpu\"]",
        "modules = [\"top\"]\n[top]\nmetrics = [\"disk\"]",
        "cache_dir = \"relative\"",
        "modules = [\"top\"]\n[top]\nwarning_style = \"#[bad]\"",
    ] {
        assert!(Sidebar::from_toml(config).is_err(), "{config}");
    }
    let sidebar =
        Sidebar::from_toml("cache_dir = \"/tmp/starmux-example\"\nmodules = [\"top\"]").unwrap();
    assert_eq!(
        sidebar.cache_dir_for("scroll").unwrap().to_str(),
        Some("/tmp/starmux-example/scroll")
    );
    assert!(sidebar.cache_dir_for("../escape").is_none());
    let disabled = Sidebar::from_toml("modules = [\"top\"]\n[top]\ndisabled = true").unwrap();
    assert!(!render(&disabled, None, 30).contains("SYSTEM"));
}
