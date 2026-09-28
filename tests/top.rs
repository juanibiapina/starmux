use starmux::{
    top::{Battery, HostStatus},
    RenderInputs, Session, Sidebar, Snapshot, Window,
};
use std::collections::BTreeMap;

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
        wide.contains("BAT") && wide.contains("19%") && wide.contains("charging"),
        "{wide}"
    );
    assert!(wide.contains("75s old") && wide.contains("--"), "{wide}");
    let narrow = render(&sidebar, Some(&status), 15);
    assert!(
        narrow.contains("19%") && !narrow.contains("charging"),
        "{narrow}"
    );
    assert!(render(&sidebar, Some(&status), 1).contains("#[nl]"));
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
