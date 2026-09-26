use starmux::{Session, Sidebar, Snapshot, Window};
use std::{collections::BTreeMap, hint::black_box, time::Instant};

fn main() {
    let windows = std::env::args()
        .nth(1)
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(200);
    let iterations = std::env::args()
        .nth(2)
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(1_000);
    let snapshot = Snapshot {
        width: 30,
        client_width: 100,
        client_height: 25,
        status_lines: 1,
        current_session: "$0".into(),
        current_pane: "%0".into(),
        pane_path: "/tmp".into(),
        sessions: vec![Session {
            id: "$0".into(),
            name: "bench".into(),
            windows: (0..windows)
                .map(|index| Window {
                    id: format!("@{index}"),
                    index,
                    name: format!("window-{index}"),
                    selected: index == 0,
                    pane: format!("%{index}"),
                    path: "/tmp".into(),
                    options: BTreeMap::new(),
                })
                .collect(),
        }],
    };
    let sidebar = Sidebar::defaults().expect("default configuration");
    let started = Instant::now();
    for _ in 0..iterations {
        black_box(sidebar.render(black_box(&snapshot)).expect("render"));
    }
    let elapsed = started.elapsed();
    println!(
        "rendered {windows} windows {iterations} times in {elapsed:?} ({:?}/render)",
        elapsed / iterations as u32
    );
}
