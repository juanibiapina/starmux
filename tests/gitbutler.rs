use starmux::{
    BuildState, GitButlerBranch, GitButlerCommit, GitButlerStatus, PushStatus, RenderInputs,
    Session, Sidebar, Snapshot,
};
use std::collections::BTreeMap;

fn snapshot(width: usize) -> Snapshot {
    Snapshot {
        width,
        client_width: 100,
        client_height: 40,
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

fn branch(name: &str, push: PushStatus) -> GitButlerBranch {
    GitButlerBranch {
        name: name.into(),
        depth: 0,
        commits: 0,
        recent_commits: Vec::new(),
        upstream_commits: 0,
        changes: 0,
        push,
        conflicted: false,
        review: None,
        ci: None,
    }
}

fn commit(id: &str, message: &str) -> GitButlerCommit {
    GitButlerCommit {
        id: id.into(),
        message: message.into(),
        conflicted: false,
    }
}

fn render(config: &str, width: usize, status: &GitButlerStatus) -> String {
    Sidebar::from_toml(config)
        .unwrap()
        .render_with_inputs(
            &snapshot(width),
            RenderInputs {
                top: None,
                pi_sessions: &[],
                usage_rows: &[],
                gob_jobs: &[],
                context: None,
                commands: &BTreeMap::new(),
                git: None,
                gitbutler: Some(status),
                debug: None,
            },
        )
        .unwrap()
}

fn visible_rows(rendered: &str) -> Vec<String> {
    rendered
        .split("#[nl]")
        .skip(2)
        .map(|row| {
            row.split("#[")
                .map(|part| part.split_once(']').map_or(part, |(_, text)| text))
                .collect::<String>()
                .replace("##", "#")
                .trim_end()
                .to_owned()
        })
        .filter(|row| !row.is_empty())
        .collect()
}

fn workspace() -> GitButlerStatus {
    GitButlerStatus {
        base: Some(commit("50e5d71", "Open builds on GitHub")),
        branches: vec![
            GitButlerBranch {
                commits: 4,
                recent_commits: vec![
                    commit("aaaaaaa", "Add module"),
                    GitButlerCommit {
                        conflicted: true,
                        ..commit("bbbbbbb", "Wire rows")
                    },
                    commit("ccccccc", "Parse status"),
                    commit("ddddddd", "Start"),
                ],
                upstream_commits: 1,
                changes: 1,
                conflicted: true,
                review: Some("42".into()),
                ci: Some(BuildState::Failure),
                ..branch("feature-top", PushStatus::NeedsForce)
            },
            GitButlerBranch {
                depth: 1,
                commits: 1,
                recent_commits: vec![commit("eeeeeee", "Base work")],
                ci: Some(BuildState::Success),
                ..branch("feature-base", PushStatus::Pushed)
            },
            GitButlerBranch {
                ci: Some(BuildState::Pending),
                ..branch("wip", PushStatus::Unpushed)
            },
            branch("local-only", PushStatus::Local),
            branch("merged", PushStatus::Integrated),
        ],
        unassigned_changes: 7,
        conflicted_files: 1,
        behind: 5,
    }
}

#[test]
fn workspace_shows_base_changes_branches_and_commits() {
    let rendered = render("modules = [\"gitbutler\"]", 50, &workspace());
    assert_eq!(
        visible_rows(&rendered),
        [
            " 󰜘 50e5d71 Open builds on GitHub ↓5 ⚠1",
            "  ◆ 7 unassigned",
            "  ⇡ feature-top 4 ◆1 #42 ✗ ⚠",
            "    ● Add module",
            "    ⚠ Wire rows",
            "    ● Parse status",
            "    … 1 more",
            "  └ ● feature-base 1 ✓",
            "    ● Base work",
            "  ↑ wip ●",
            "  ○ local-only",
            "  ✔ merged",
        ],
        "{rendered}"
    );
    assert!(rendered.contains("#[fg=red]⇡"), "{rendered}");
}

#[test]
fn commit_lists_can_be_limited() {
    let rendered = render(
        "modules = [\"gitbutler\"]\n[gitbutler]\nmax_commits = 1\n",
        50,
        &workspace(),
    );
    let rows = visible_rows(&rendered);
    assert!(rows.contains(&"    ● Add module".to_owned()), "{rows:?}");
    assert!(rows.contains(&"    … 3 more".to_owned()), "{rows:?}");
    assert!(
        !rows.iter().any(|row| row.contains("Wire rows")),
        "{rows:?}"
    );
}

#[test]
fn branch_names_and_messages_render_as_literal_text() {
    let status = GitButlerStatus {
        branches: vec![GitButlerBranch {
            commits: 1,
            recent_commits: vec![commit("aaaaaaa", "#[range=user|evil]msg")],
            ..branch("#[range=user|evil]#{pane_id}", PushStatus::Local)
        }],
        ..GitButlerStatus::default()
    };
    let rendered = render("modules = [\"gitbutler\"]", 50, &status);
    for text in ["##[range=user|evil]##{pane_id}", "##[range=user|evil]msg"] {
        assert!(rendered.contains(text), "{text}: {rendered}");
    }
    assert!(!rendered.contains(" #[range=user|evil]"), "{rendered}");
}

#[test]
fn heading_is_omitted_without_a_base_or_counts() {
    let status = GitButlerStatus {
        branches: vec![branch("solo", PushStatus::Local)],
        ..GitButlerStatus::default()
    };
    let rendered = render("modules = [\"gitbutler\"]", 40, &status);
    assert_eq!(visible_rows(&rendered), ["  ○ solo"], "{rendered}");
    let empty = render("modules = [\"gitbutler\"]", 40, &GitButlerStatus::default());
    assert!(visible_rows(&empty).is_empty(), "{empty}");
}

#[test]
fn slim_layout_shows_unassigned_changes_and_branch_states() {
    let rendered = render("modules = [\"gitbutler\"]", 2, &workspace());
    assert_eq!(
        visible_rows(&rendered),
        ["◆·", "⇡⚠", "●✓", "↑●", "○·", "✔·"],
        "{rendered}"
    );
}

#[test]
fn formats_and_styles_are_configurable_and_validated() {
    let rendered = render(
        "palette = \"mine\"\nmodules = [\"gitbutler\"]\n[gitbutler]\nheading_format = \" Branches\"\nformat = \" $name( $upstream)\"\ncommit_format = \"   $id $message\"\nheading_style = \"fg=accent\"\n[palettes.mine]\naccent = \"#123456\"\n",
        40,
        &workspace(),
    );
    assert!(rendered.contains("#[fg=#123456] Branches"), "{rendered}");
    let rows = visible_rows(&rendered);
    assert!(rows.contains(&" feature-top ⇣1".to_owned()), "{rows:?}");
    assert!(
        rows.contains(&"   aaaaaaa Add module".to_owned()),
        "{rows:?}"
    );
    for invalid in [
        "[gitbutler]\nunknown = 1",
        "[gitbutler]\nformat = \"$missing\"",
        "[gitbutler]\ncommit_format = \"$name\"",
        "[gitbutler]\nforce_style = \"fg=nope\"",
        "[gitbutler]\nmax_commits = 21",
    ] {
        assert!(Sidebar::from_toml(invalid).is_err(), "{invalid}");
    }
}
