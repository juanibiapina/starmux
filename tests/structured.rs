use std::{fs, process::Command};

#[test]
fn structured_provider_uses_semantic_styles_and_escapes_literal_text() {
    let root = std::env::temp_dir().join(format!("sidebar-json-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let fixture = root.join("data.json");
    fs::write(&fixture, r##"{"version":1,"rows":[{"segments":[{"text":"#[fg=red] #{pane_id}","style":"detail","fill":"highlight"}]}],"metadata":null}"##).unwrap();
    let config = root.join("config.toml");
    fs::write(&config, format!("format = \"$sessions$custom\"\n[provider.demo]\ncommand = [\"/bin/cat\", \"{}\"]\ncwd = \"/tmp\"\ndecoder = \"json-v1\"\ncache = false\n[module.custom]\nprovider = \"demo\"\n", fixture.display())).unwrap();
    let args = [
        "module",
        "custom",
        "--input-version=1",
        "--width=30",
        "--client-width=80",
        "--client-height=24",
        "--current-session=$0",
        "--current-pane=%0",
        "--pane-path=/tmp",
        "--session-count=1",
        "--session-id=$0",
        "--session-name=test",
        "--window-count=0",
        "--end-session",
    ];
    let out = Command::new(env!("CARGO_BIN_EXE_starmux"))
        .args(args)
        .env("STARMUX_CONFIG", &config)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        stdout.contains("#[fg=cyan,bold]##[fg=red] ##{pane_id}"),
        "{stdout}"
    );
    assert!(stdout.contains("#[fill=#292e42]"), "{stdout}");
    fs::remove_dir_all(root).unwrap();
}
