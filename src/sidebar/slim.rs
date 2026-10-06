use super::*;

const UNKNOWN: &str = "󰋗";
const WARNING: &str = "";
const SIGN_IN: &str = "󰌾";

impl Sidebar {
    pub(super) fn slim_rows(
        &self,
        module: &str,
        rows: Vec<Row>,
        snapshot: &Snapshot,
        input: RenderInputs<'_>,
    ) -> Result<Vec<Row>, String> {
        if module == "git" && !self.config.git.disabled {
            return input
                .git
                .map_or_else(|| Ok(Vec::new()), |status| self.slim_git(status));
        }
        let mut pi = if module == "pi-workbench" {
            input.pi_sessions.to_vec()
        } else {
            Vec::new()
        };
        crate::pi_workbench::sort_sessions(&mut pi);
        let mut pi = pi.iter();
        let mut windows = BTreeMap::<String, usize>::new();
        let mut result = Vec::new();
        for mut row in rows {
            let kind = row.identity.get("kind");
            if module == "usage"
                && kind == "provider"
                && input
                    .usage_rows
                    .iter()
                    .find(|reading| reading.provider == row.identity.get("provider"))
                    .is_some_and(|reading| {
                        reading.windows.iter().any(|window| {
                            window.used_percent.is_finite()
                                && (0.0..=100.0).contains(&window.used_percent)
                        })
                    })
            {
                continue;
            }
            if matches!(
                kind,
                "skill" | "heading" | "category" | "project" | "cache-age" | "stage"
            ) {
                continue;
            }
            match module {
                "sessions" => self.slim_session(&mut row, snapshot)?,
                "pi-workbench" => {
                    if let Some(session) = pi.next() {
                        self.slim_pi(&mut row, session)?;
                    }
                }
                "pi-context" => self.slim_context(&mut row, input)?,
                "usage" => self.slim_usage(&mut row, input.usage_rows, &mut windows)?,
                "top" => self.slim_metric(&mut row, input.top)?,
                "gob" => self.slim_job(&mut row, input.gob_jobs)?,
                "debug" => self.slim_text(
                    &mut row,
                    if input.debug.is_some() {
                        "◷"
                    } else {
                        UNKNOWN
                    },
                ),
                "divider" => {
                    let glyph = &self.config.divider.character;
                    let glyph = if UnicodeWidthStr::width(glyph.as_str()) == 1 {
                        glyph.as_str()
                    } else {
                        "─"
                    };
                    self.slim_text(&mut row, &glyph.repeat(2));
                }
                name if name.starts_with("command.") => {
                    let config = &self.config.commands[&name[8..]];
                    row.spans = vec![Span {
                        text: config.slim_icon.clone(),
                        style: resolve_style(&config.style, "default", &self.palette)?,
                    }];
                }
                _ => {}
            }
            result.push(row);
        }
        Ok(result)
    }

    fn slim_text(&self, row: &mut Row, text: &str) {
        let style = row
            .spans
            .first()
            .map_or_else(|| "default".into(), |span| span.style.clone());
        row.spans = vec![Span {
            text: text.into(),
            style,
        }];
    }

    fn slim_session(&self, row: &mut Row, snapshot: &Snapshot) -> Result<(), String> {
        let cfg = &self.sessions.config;
        let session = row.identity.get("kind") == "session";
        let current = row.identity.get("target_session") == snapshot.current_session;
        let style = if session {
            if current {
                &cfg.current_session_style
            } else {
                &cfg.other_session_style
            }
        } else if row.focus {
            &cfg.active_window_style
        } else if row.selected {
            &cfg.selected_window_style
        } else {
            &cfg.other_window_style
        };
        let selected = if session { current } else { row.selected };
        row.spans = vec![Span {
            text: format!(
                "{}{}",
                if session { "󰆍" } else { "" },
                if selected { "●" } else { "○" }
            ),
            style: resolve_style(style, "default", &self.palette)?,
        }];
        Ok(())
    }

    fn slim_pi(&self, row: &mut Row, session: &crate::PiSession) -> Result<(), String> {
        let cfg = &self.config.pi_workbench;
        let (glyph, style) = match session.state.as_str() {
            "notify" => ("󰂚", &cfg.notify_style),
            "working" => ("▶", &cfg.working_style),
            _ => ("○", &cfg.idle_style),
        };
        let mut style = resolve_style(style, "default", &self.palette)?;
        if session.selected {
            style = format!(
                "{style},bg={}",
                resolve_color(&cfg.selected_fill, &self.palette)?
            );
        }
        row.spans = vec![Span {
            text: glyph.into(),
            style,
        }];
        row.spans.push(Span {
            text: if session.selected { "●" } else { "○" }.into(),
            style: resolve_style(
                if session.selected {
                    &cfg.selected_style
                } else {
                    "default"
                },
                "default",
                &self.palette,
            )?,
        });
        Ok(())
    }

    fn slim_context(&self, row: &mut Row, input: RenderInputs<'_>) -> Result<(), String> {
        // Full context item rows already carry their semantic icon and style.
        row.spans = row
            .spans
            .iter()
            .find(|span| !span.text.trim().is_empty())
            .cloned()
            .into_iter()
            .collect();
        if row.identity.get("kind") == "pr" {
            let state = input
                .context
                .and_then(|context| {
                    context
                        .pull_requests
                        .iter()
                        .position(|url| url == row.identity.get("url"))
                })
                .and_then(|index| input.states.get(index))
                .copied()
                .unwrap_or(crate::pr_state::PrState::Unknown);
            if state == crate::pr_state::PrState::Unknown {
                row.spans.push(Span {
                    text: UNKNOWN.into(),
                    style: resolve_style(
                        &self.config.pi_context.unknown_style,
                        "default",
                        &self.palette,
                    )?,
                });
            }
        }
        Ok(())
    }

    fn slim_usage(
        &self,
        row: &mut Row,
        readings: &[crate::usage::UsageRow],
        positions: &mut BTreeMap<String, usize>,
    ) -> Result<(), String> {
        let provider = row.identity.get("provider");
        let Some(reading) = readings.iter().find(|r| r.provider == provider) else {
            return Ok(());
        };
        let cfg = &self.config.usage;
        if row.identity.get("kind") == "provider" {
            let badge = provider_icon(provider);
            let state = match reading.refresh_failure {
                Some(crate::usage::RefreshFailure::SignInAgain) => SIGN_IN,
                Some(crate::usage::RefreshFailure::RefreshFailed) => WARNING,
                None if reading.unavailable => UNKNOWN,
                None if reading.stale => "◷",
                None => "",
            };
            let text = format!("{badge}{state}");
            self.slim_text(row, &text);
            return Ok(());
        }
        let position = positions.entry(provider.into()).or_default();
        let window = reading
            .windows
            .iter()
            .filter(|w| w.used_percent.is_finite() && (0.0..=100.0).contains(&w.used_percent))
            .nth(*position);
        *position += 1;
        let Some(window) = window else {
            return Ok(());
        };
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let style = if window.used_percent >= 80.0 {
            &cfg.critical_bar_style
        } else if quota_behind_time(window, now) {
            &cfg.warning_bar_style
        } else {
            &cfg.window_style
        };
        let mut style = resolve_style(style, "default", &self.palette)?;
        if reading.stale || reading.refresh_failure.is_some() {
            style = format!(
                "{style},{}",
                resolve_style(&cfg.stale_style, "default", &self.palette)?
            );
        }
        let badge = match reading.refresh_failure {
            Some(crate::usage::RefreshFailure::SignInAgain) => SIGN_IN,
            Some(crate::usage::RefreshFailure::RefreshFailed) => WARNING,
            None => provider_icon(provider),
        };
        row.spans = vec![Span {
            text: format!("{}{}", badge, percentage_pie(window.used_percent)),
            style,
        }];
        Ok(())
    }

    fn slim_metric(
        &self,
        row: &mut Row,
        status: Option<&crate::top::HostStatus>,
    ) -> Result<(), String> {
        let metric = row.identity.get("metric");
        let battery = status.and_then(|s| s.battery.as_ref());
        let (glyph, value) = match metric {
            "cpu" => ("󰻠", status.and_then(|s| s.cpu)),
            "memory" => ("󰍛", status.and_then(|s| s.memory)),
            _ => (
                battery.map_or("󰁹", |b| {
                    if b.full {
                        "󱟢"
                    } else if b.charging {
                        "󰂄"
                    } else if b.percent <= 20 {
                        "󰁺"
                    } else {
                        "󰁹"
                    }
                }),
                battery.map(|b| b.percent),
            ),
        };
        let critical = value.is_some_and(|v| {
            if metric == "battery" {
                v <= 20
            } else {
                v >= 80
            }
        });
        let stale = status.is_some_and(|s| s.age_seconds.is_some());
        let cfg = &self.config.top;
        let style = if stale || value.is_none() {
            &cfg.track_style
        } else if critical {
            &cfg.critical_style
        } else {
            &cfg.value_style
        };
        let mut style = resolve_style(style, "default", &self.palette)?;
        if critical && !stale {
            style.push_str(",bold");
        }
        let text = format!(
            "{glyph}{}",
            value.map_or(UNKNOWN, |v| percentage_pie(f64::from(v)))
        );
        row.spans = vec![Span { text, style }];
        Ok(())
    }

    fn slim_job(&self, row: &mut Row, jobs: &[crate::GobJob]) -> Result<(), String> {
        if row.identity.get("part") == "progress" {
            let progress = jobs
                .iter()
                .find(|j| j.id == row.identity.get("job_id"))
                .and_then(|j| j.progress(time::OffsetDateTime::now_utc()));
            let (glyph, style) = match progress.map(|p| (p, p.phase())) {
                None => (UNKNOWN, &self.config.gob.progress_style),
                Some((_, crate::gob::Phase::Overdue { .. })) => {
                    ("█", &self.config.gob.overdue_style)
                }
                Some((p, _)) => (
                    gauge(p.elapsed.as_secs_f64() / p.typical.as_secs_f64() * 100.0),
                    &self.config.gob.progress_style,
                ),
            };
            row.spans = vec![Span {
                text: format!("↳{glyph}"),
                style: resolve_style(style, "default", &self.palette)?,
            }];
        } else {
            row.spans = vec![Span {
                text: "▶".into(),
                style: resolve_style(&self.config.gob.running_style, "default", &self.palette)?,
            }];
        }
        Ok(())
    }

    fn slim_git(&self, status: &crate::GitStatus) -> Result<Vec<Row>, String> {
        let cfg = &self.config.git;
        let (glyph, style) = if status.conflicts > 0 {
            (WARNING, &cfg.conflicts_style)
        } else if !status.state.is_empty() {
            ("↻", &cfg.state_style)
        } else if !status.clean() {
            ("", &cfg.modified_style)
        } else {
            ("✓", &cfg.clean_style)
        };
        let mut spans = vec![Span {
            text: glyph.into(),
            style: resolve_style(style, "default", &self.palette)?,
        }];
        spans.push(Span {
            text: match (status.ahead > 0, status.behind > 0) {
                (true, true) => "↕",
                (true, false) => "↑",
                (false, true) => "↓",
                _ => "·",
            }
            .into(),
            style: resolve_style(&cfg.divergence_style, "default", &self.palette)?,
        });
        Ok(vec![Row {
            spans,
            identity: crate::actions::Identity::new("summary"),
            ..Row::blank()
        }])
    }
}

fn provider_icon(provider: &str) -> &'static str {
    match provider {
        "anthropic" => "󰚩",
        "codex" => "",
        "copilot" => "",
        "gemini" => "✦",
        "antigravity" => "◎",
        "kiro" => "󰊠",
        "zai" => "󰘦",
        "xai" => "󰖟",
        _ => UNKNOWN,
    }
}

fn percentage_pie(percent: f64) -> &'static str {
    const STEPS: [&str; 5] = ["○", "◔", "◑", "◕", "●"];
    STEPS[(percent.clamp(0.0, 100.0) / 25.0).round() as usize]
}

fn gauge(percent: f64) -> &'static str {
    if percent <= 0.0 {
        return "░";
    }
    const LEVELS: [&str; 8] = ["▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];
    LEVELS[((percent.clamp(0.0, 100.0) / 12.5).ceil() as usize)
        .saturating_sub(1)
        .min(7)]
}
