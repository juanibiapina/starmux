use crate::input::Context;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Style {
    SessionCurrent,
    SessionOther,
    WindowActive,
    WindowSelected,
    WindowOther,
    MarkerWorking,
    MarkerNotify,
    Divider,
    Footer,
    Detail,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub style: Style,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub spans: Vec<Span>,
    pub fill: Option<String>,
    pub range: Option<Range>,
    pub focus: bool,
    pub selected: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Range {
    Session(String),
    Window(usize),
    ForeignWindow(String),
}

impl Row {
    pub fn new() -> Self {
        Self {
            spans: Vec::new(),
            fill: None,
            range: None,
            focus: false,
            selected: false,
        }
    }
    pub fn text(mut self, text: impl Into<String>, style: Style) -> Self {
        self.spans.push(Span {
            text: text.into(),
            style,
        });
        self
    }
}

pub fn sessions(ctx: &Context) -> Result<Vec<Row>, String> {
    let mut rows = Vec::new();
    for session in &ctx.sessions {
        let current = session.id == ctx.current_session;
        let color = if current { "#c099ff" } else { "#292e42" };
        let style = if current {
            Style::SessionCurrent
        } else {
            Style::SessionOther
        };
        let mut row = Row::new().text(format!(" {} ", session.name), style);
        row.range = Some(Range::Session(session.id.clone()));
        row.fill = Some(color.into());
        rows.push(row);
        for window in &session.windows {
            let active = current && window.selected;
            let style = if active {
                Style::WindowActive
            } else if window.selected {
                Style::WindowSelected
            } else {
                Style::WindowOther
            };
            let mut row = Row::new().text(format!(" {} ", window.index), style.clone());
            match window.pi_state.as_str() {
                "working" => {
                    row = row.text("●", Style::MarkerWorking);
                }
                "notify" => {
                    row = row.text("●", Style::MarkerNotify);
                }
                _ => {
                    row = row.text(
                        if window.icon.is_empty() {
                            "|"
                        } else {
                            &window.icon
                        },
                        style.clone(),
                    );
                }
            }
            row = row.text(format!(" {}", window.name), style);
            row.range = Some(if current {
                Range::Window(window.index)
            } else {
                Range::ForeignWindow(crate::navigation::token(&session.id, &window.id)?)
            });
            row.focus = active;
            row.selected = window.selected;
            row.fill = Some(if active { "#3b4261" } else { "#1b1d2b" }.into());
            rows.push(row);
        }
    }
    Ok(rows)
}

pub fn divider(width: usize) -> Vec<Row> {
    vec![Row::new().text(
        format!(" {}", "─".repeat(width.saturating_sub(2))),
        Style::Divider,
    )]
}

fn escaped(text: &str) -> String {
    text.chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .collect::<String>()
        .replace('#', "##")
}

fn clipped(text: &str, remaining: &mut usize) -> String {
    let mut out = String::new();
    for grapheme in text.graphemes(true) {
        let width = UnicodeWidthStr::width(grapheme);
        if width > *remaining {
            break;
        }
        *remaining -= width;
        out.push_str(grapheme);
    }
    out
}

fn style(style: &Style) -> &'static str {
    match style {
        Style::SessionCurrent => "#[fg=#1b1d2b,bg=#c099ff,bold]",
        Style::SessionOther => "#[fg=#82aaff,bg=#292e42,bold]",
        Style::WindowActive => "#[fg=#c099ff,bg=#3b4261,bold,nounderscore,noitalics]",
        Style::WindowSelected => "#[fg=#828bb8,bg=#1e2030,bold,nounderscore,noitalics]",
        Style::WindowOther => "#[fg=#828bb8,bg=#1e2030,nobold,nounderscore,noitalics]",
        Style::MarkerWorking => "#[fg=#ffc777]",
        Style::MarkerNotify => "#[fg=#c099ff]",
        Style::Divider => "#[fg=#3b4261,nobold]",
        Style::Footer => "#[fg=#82aaff,bold]",
        Style::Detail => "#[fg=cyan,bold]",
    }
}

pub fn render(rows: &[Row], width: usize) -> String {
    let mut out = String::from(
        "#[list=on]#[list=left-marker]#[acs]-#[noacs]#[nl]#[list=right-marker]#[acs].#[noacs]#[nl]",
    );
    for row in rows {
        let mut remaining = width;
        if let Some(range) = &row.range {
            match range {
                Range::Session(id) => out.push_str(&format!("#[range=session|{id} ]")),
                Range::Window(index) => out.push_str(&format!(
                    "#[range=window|{index} {}]",
                    if row.focus { "list=focus " } else { "" }
                )),
                Range::ForeignWindow(token) => out.push_str(&format!("#[range=user|{token} ]")),
            }
        }
        for span in &row.spans {
            let text = clipped(&span.text, &mut remaining);
            if text.is_empty() {
                continue;
            }
            out.push_str(style(&span.style));
            out.push_str(&escaped(&text));
        }
        if row.range.is_some() {
            if let Some(fill) = &row.fill {
                out.push_str(&format!("#[bg={fill}]"));
            }
            out.push_str(&" ".repeat(remaining.saturating_sub(1)));
        }
        match row.range {
            Some(Range::Session(_)) => out.push_str("#[norange default]"),
            Some(Range::Window(_) | Range::ForeignWindow(_)) if row.selected => {
                out.push_str("#[norange]#[list=on default]")
            }
            Some(Range::Window(_) | Range::ForeignWindow(_)) => out.push_str("#[norange default]"),
            None => {}
        }
        if let Some(fill) = &row.fill {
            out.push_str(&format!("#[fill={fill}]"));
        }
        out.push_str("#[nl]");
        if row.fill.is_some() {
            out.push_str("#[fill=#1b1d2b]");
        }
    }
    out
}
