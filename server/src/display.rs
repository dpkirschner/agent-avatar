//! The display: what the kiosk page shows besides the avatar's state.
//!
//! A preset (how the avatar and the content share the stage), up to
//! [`MAX_CARDS`] typed content cards, and the two status lines. The agent
//! sends a [`DisplayPatch`] to `POST /display`; every card and status line
//! is checked here before anything changes, so the page only ever gets
//! data it knows how to draw. Cards carry plain text and numbers, never
//! HTML or URLs: a `screen` card names a configured screen, and its URL
//! is built here from the screens config and the screen's allowed params.

use crate::config::{ParamRule, Screen};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Most cards on the stage at once.
pub const MAX_CARDS: usize = 3;
/// Most points in one chart card, over all its series.
pub const MAX_CHART_POINTS: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Preset {
    /// Avatar wide, content narrow.
    #[default]
    Chat,
    /// About half each.
    Split,
    /// Avatar a narrow column, content dominant.
    Focus,
}

/// What the avatar is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Avatar {
    #[default]
    Idle,
    Listening,
    Thinking,
    Talking,
}

/// What the page shows, as sent to it in a `display` event.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Display {
    pub preset: Preset,
    pub content: Vec<Card>,
    pub status: Status,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Status {
    pub top: Option<Bar>,
    pub bottom: Option<Bar>,
}

/// One status line: a built-in template and the values for its slots.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bar {
    pub template: String,
    #[serde(default)]
    pub items: BTreeMap<String, Value>,
}

/// The body of `POST /display`. Each field sent replaces that part;
/// fields left out keep what is there.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DisplayPatch {
    pub preset: Option<Preset>,
    pub avatar: Option<Avatar>,
    /// Raw, so a bad card's error can name its index.
    pub content: Option<Vec<Value>>,
    pub status: Option<StatusPatch>,
}

/// `top` / `bottom`: a bar replaces the line, `null` clears it, absent
/// keeps it.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatusPatch {
    #[serde(default, deserialize_with = "present")]
    pub top: Option<Option<Bar>>,
    #[serde(default, deserialize_with = "present")]
    pub bottom: Option<Option<Bar>>,
}

/// Tells a field sent as `null` (`Some(None)`) from one left out (`None`).
fn present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Option<T>, D::Error> {
    T::deserialize(d).map(Some)
}

/// A checked patch, ready to apply.
#[derive(Debug, Default)]
pub struct Checked {
    pub preset: Option<Preset>,
    pub avatar: Option<Avatar>,
    pub content: Option<Vec<Card>>,
    pub top: Option<Option<Bar>>,
    pub bottom: Option<Option<Bar>>,
}

impl Checked {
    /// Whether anything besides the avatar changes.
    pub fn touches_display(&self) -> bool {
        self.preset.is_some()
            || self.content.is_some()
            || self.top.is_some()
            || self.bottom.is_some()
    }
}

/// Check a patch against the card schemas, the status templates and the
/// screens config. The error names the part that failed.
pub fn check(patch: DisplayPatch, screens: &[Screen]) -> Result<Checked, String> {
    let content = match patch.content {
        None => None,
        Some(cards) => {
            if cards.len() > MAX_CARDS {
                return Err(format!("content: at most {MAX_CARDS} cards"));
            }
            let mut out = Vec::with_capacity(cards.len());
            for (i, v) in cards.into_iter().enumerate() {
                out.push(check_card(v, screens).map_err(|e| format!("content[{i}]: {e}"))?);
            }
            Some(out)
        }
    };
    let (top, bottom) = match patch.status {
        None => (None, None),
        Some(s) => {
            for (name, bar) in [("top", &s.top), ("bottom", &s.bottom)] {
                if let Some(Some(b)) = bar {
                    check_bar(b).map_err(|e| format!("status.{name}: {e}"))?;
                }
            }
            (s.top, s.bottom)
        }
    };
    Ok(Checked {
        preset: patch.preset,
        avatar: patch.avatar,
        content,
        top,
        bottom,
    })
}

impl Display {
    /// Apply a checked patch. New cards without a preset leave `chat` for
    /// `split` (another preset stays); clearing the cards falls back to
    /// `chat`. An explicit preset always wins.
    pub fn apply(&mut self, c: Checked) {
        if let Some(content) = c.content {
            if c.preset.is_none() {
                if content.is_empty() {
                    self.preset = Preset::Chat;
                } else if self.preset == Preset::Chat {
                    self.preset = Preset::Split;
                }
            }
            self.content = content;
        }
        if let Some(p) = c.preset {
            self.preset = p;
        }
        if let Some(t) = c.top {
            self.status.top = t;
        }
        if let Some(b) = c.bottom {
            self.status.bottom = b;
        }
    }
}

// ---------- status lines ----------

#[derive(Debug, Clone, Copy)]
enum Slot {
    /// A whole number, 0-9999.
    Count,
    /// Text up to this many characters.
    Text(usize),
}

/// The built-in status templates and their slots. The page draws each
/// template its own way (`app/app.js`, `STATUS_TEMPLATES`); keep the two
/// lists in step.
const TEMPLATES: &[(&str, &[(&str, Slot)])] = &[
    (
        "desk",
        &[
            ("alerts", Slot::Count),
            ("mail", Slot::Count),
            ("next", Slot::Text(60)),
        ],
    ),
    ("note", &[("text", Slot::Text(120))]),
];

fn check_bar(b: &Bar) -> Result<(), String> {
    let Some((_, slots)) = TEMPLATES.iter().find(|(n, _)| *n == b.template) else {
        let names: Vec<&str> = TEMPLATES.iter().map(|(n, _)| *n).collect();
        return Err(format!(
            "unknown template {:?} (have {})",
            b.template,
            names.join(", ")
        ));
    };
    for (key, v) in &b.items {
        let Some((_, slot)) = slots.iter().find(|(n, _)| n == key) else {
            let names: Vec<&str> = slots.iter().map(|(n, _)| *n).collect();
            return Err(format!(
                "template {:?} has no slot {key:?} (has {})",
                b.template,
                names.join(", ")
            ));
        };
        match slot {
            Slot::Count => {
                if !v.as_u64().is_some_and(|n| n <= 9999) {
                    return Err(format!("{key}: a whole number 0-9999"));
                }
            }
            Slot::Text(max) => match v.as_str() {
                Some(s) => text(key, s, *max)?,
                None => return Err(format!("{key}: text")),
            },
        }
    }
    Ok(())
}

// ---------- cards ----------

/// A number or a short text, as the agent sent it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Scalar {
    Num(serde_json::Number),
    Text(String),
}

impl Scalar {
    fn check(&self, what: &str, max: usize) -> Result<(), String> {
        match self {
            Scalar::Num(n) if n.as_f64().is_some_and(f64::is_finite) => Ok(()),
            Scalar::Num(_) => Err(format!("{what}: not a finite number")),
            Scalar::Text(s) => text(what, s, max),
        }
    }

    fn as_param(&self) -> String {
        match self {
            Scalar::Num(n) => n.to_string(),
            Scalar::Text(s) => s.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Card {
    Text(TextCard),
    List(ListCard),
    Figures(FiguresCard),
    Alerts(AlertsCard),
    Calendar(CalendarCard),
    Goals(GoalsCard),
    Chart(ChartCard),
    Transcript(TranscriptCard),
    Screen(ScreenCard),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextCard {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub body: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mark {
    Done,
    Open,
    Warn,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListCard {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub items: Vec<ListItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListItem {
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mark: Option<Mark>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tone {
    Good,
    Bad,
    Neutral,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FiguresCard {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub items: Vec<Figure>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Figure {
    pub label: String,
    pub value: Scalar,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delta: Option<Scalar>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tone: Option<Tone>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Crit,
    Warn,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AlertsCard {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub items: Vec<Alert>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Alert {
    pub level: Level,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CalendarCard {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub events: Vec<CalendarEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CalendarEvent {
    pub start: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<String>,
    pub title: String,
    #[serde(rename = "where", default, skip_serializing_if = "Option::is_none")]
    pub place: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub all_day: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoalsCard {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub items: Vec<Goal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Goal {
    pub name: String,
    /// 0 to 1.
    pub progress: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current: Option<Scalar>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<Scalar>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChartKind {
    Line,
    Bar,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChartCard {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub kind: ChartKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    pub series: Vec<Series>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub highlight: Option<Highlight>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Series {
    pub name: String,
    /// `[x, y]`: x a number, a time, or a category; y a number.
    pub points: Vec<(Scalar, f64)>,
}

/// A range (`from` and `to`) or a point (`at`) on the x axis.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Highlight {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<Scalar>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<Scalar>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<Scalar>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TranscriptCard {
    /// How many recent lines to show; all the page keeps when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lines: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScreenCard {
    pub name: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, Scalar>,
    /// Filled in here from the screens config; the agent can't send it.
    #[serde(skip_deserializing, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_deserializing, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

const TITLE: usize = 80;
const SHORT: usize = 40;
const LINE: usize = 200;
const DETAIL: usize = 300;
const VALUE: usize = 24;
const UNIT: usize = 12;

fn check_card(v: Value, screens: &[Screen]) -> Result<Card, String> {
    let mut card: Card = serde_json::from_value(v).map_err(|e| e.to_string())?;
    match &mut card {
        Card::Text(c) => {
            opt_text("title", &c.title, TITLE)?;
            text("body", &c.body, 2000)?;
        }
        Card::List(c) => {
            opt_text("title", &c.title, TITLE)?;
            count("items", c.items.len(), 12)?;
            for it in &c.items {
                text("text", &it.text, LINE)?;
                opt_text("detail", &it.detail, DETAIL)?;
            }
        }
        Card::Figures(c) => {
            opt_text("title", &c.title, TITLE)?;
            count("items", c.items.len(), 6)?;
            for f in &c.items {
                text("label", &f.label, SHORT)?;
                f.value.check("value", VALUE)?;
                opt_text("unit", &f.unit, UNIT)?;
                if let Some(d) = &f.delta {
                    d.check("delta", VALUE)?;
                }
            }
        }
        Card::Alerts(c) => {
            opt_text("title", &c.title, TITLE)?;
            count("items", c.items.len(), 8)?;
            for a in &c.items {
                text("title", &a.title, 120)?;
                opt_text("detail", &a.detail, DETAIL)?;
                opt_time("since", &a.since)?;
            }
        }
        Card::Calendar(c) => {
            opt_text("title", &c.title, TITLE)?;
            count("events", c.events.len(), 10)?;
            for e in &c.events {
                time("start", &e.start)?;
                opt_time("end", &e.end)?;
                text("title", &e.title, 120)?;
                opt_text("where", &e.place, TITLE)?;
            }
        }
        Card::Goals(c) => {
            opt_text("title", &c.title, TITLE)?;
            count("items", c.items.len(), 6)?;
            for g in &c.items {
                text("name", &g.name, TITLE)?;
                if !(0.0..=1.0).contains(&g.progress) {
                    return Err("progress: from 0 to 1".into());
                }
                if let Some(s) = &g.current {
                    s.check("current", VALUE)?;
                }
                if let Some(s) = &g.target {
                    s.check("target", VALUE)?;
                }
                opt_time("due", &g.due)?;
            }
        }
        Card::Chart(c) => check_chart(c)?,
        Card::Transcript(c) => {
            if c.lines.is_some_and(|n| !(1..=50).contains(&n)) {
                return Err("lines: from 1 to 50".into());
            }
        }
        Card::Screen(c) => {
            let s = screens
                .iter()
                .find(|s| s.name == c.name)
                .ok_or_else(|| format!("no screen named {:?}", c.name))?;
            c.url = Some(screen_url(s, &c.params)?);
            c.title = Some(s.title.clone());
        }
    }
    Ok(card)
}

fn check_chart(c: &ChartCard) -> Result<(), String> {
    opt_text("title", &c.title, TITLE)?;
    opt_text("unit", &c.unit, UNIT)?;
    if c.series.is_empty() || c.series.len() > 4 {
        return Err("series: 1 to 4".into());
    }
    let mut total = 0;
    let mut numeric_x = None;
    for s in &c.series {
        text("name", &s.name, SHORT)?;
        if s.points.is_empty() {
            return Err(format!("series {:?}: no points", s.name));
        }
        total += s.points.len();
        for (x, y) in &s.points {
            x.check("x", SHORT)?;
            if !y.is_finite() {
                return Err("y: not a finite number".into());
            }
            let num = matches!(x, Scalar::Num(_));
            if *numeric_x.get_or_insert(num) != num {
                return Err("x: all numbers or all text, not both".into());
            }
        }
    }
    if total > MAX_CHART_POINTS {
        return Err(format!("at most {MAX_CHART_POINTS} points"));
    }
    if let Some(h) = &c.highlight {
        match (&h.from, &h.to, &h.at) {
            (Some(a), Some(b), None) => {
                a.check("highlight.from", SHORT)?;
                b.check("highlight.to", SHORT)?;
            }
            (None, None, Some(a)) => a.check("highlight.at", SHORT)?,
            _ => return Err("highlight: {from, to} or {at}".into()),
        }
    }
    Ok(())
}

/// The screen's URL with the agent's params, each checked against the
/// screen's allow-list, appended as a query string.
fn screen_url(s: &Screen, params: &BTreeMap<String, Scalar>) -> Result<String, String> {
    let mut url = s.url.clone();
    for (key, v) in params {
        let rule = s
            .params
            .get(key)
            .ok_or_else(|| format!("screen {:?} takes no param {key:?}", s.name))?;
        let val = v.as_param();
        let ok = match rule {
            ParamRule::OneOf(allowed) => allowed.contains(&val),
            ParamRule::Kind(kind) => kind.accepts(&val),
        };
        if !ok {
            return Err(format!("param {key}: {val:?} is not allowed"));
        }
        url.push(if url.contains('?') { '&' } else { '?' });
        url.push_str(&encode(key));
        url.push('=');
        url.push_str(&encode(&val));
    }
    Ok(url)
}

/// Percent-encode everything but unreserved characters.
fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

// ---------- field checks ----------

fn text(what: &str, s: &str, max: usize) -> Result<(), String> {
    if s.trim().is_empty() {
        return Err(format!("{what}: empty"));
    }
    if s.chars().count() > max {
        return Err(format!("{what}: longer than {max} characters"));
    }
    Ok(())
}

fn opt_text(what: &str, s: &Option<String>, max: usize) -> Result<(), String> {
    s.as_deref().map_or(Ok(()), |s| text(what, s, max))
}

fn count(what: &str, n: usize, max: usize) -> Result<(), String> {
    if n == 0 || n > max {
        return Err(format!("{what}: 1 to {max}"));
    }
    Ok(())
}

fn time(what: &str, s: &str) -> Result<(), String> {
    if is_time(s) {
        Ok(())
    } else {
        Err(format!(
            "{what}: {s:?} is not an ISO 8601 date or time (2026-11-14, 2026-11-14T09:30, ...)"
        ))
    }
}

fn opt_time(what: &str, s: &Option<String>) -> Result<(), String> {
    s.as_deref().map_or(Ok(()), |s| time(what, s))
}

fn digits(s: &[u8]) -> bool {
    !s.is_empty() && s.iter().all(u8::is_ascii_digit)
}

/// `YYYY-MM-DD`.
fn is_date(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10
        && digits(&b[0..4])
        && b[4] == b'-'
        && digits(&b[5..7])
        && b[7] == b'-'
        && digits(&b[8..10])
}

/// `YYYY-MM-DD`, optionally `THH:MM[:SS[.fff]]` and `Z` or `±HH:MM`.
fn is_time(s: &str) -> bool {
    if s.len() < 10 || !is_date(&s[..10]) {
        return false;
    }
    let rest = &s[10..];
    if rest.is_empty() {
        return true;
    }
    let Some(rest) = rest.strip_prefix('T') else {
        return false;
    };
    let b = rest.as_bytes();
    if b.len() < 5 || !digits(&b[0..2]) || b[2] != b':' || !digits(&b[3..5]) {
        return false;
    }
    let mut i = 5;
    if b.get(i) == Some(&b':') {
        if b.len() < i + 3 || !digits(&b[i + 1..i + 3]) {
            return false;
        }
        i += 3;
        if b.get(i) == Some(&b'.') {
            let n = b[i + 1..].iter().take_while(|c| c.is_ascii_digit()).count();
            if n == 0 {
                return false;
            }
            i += 1 + n;
        }
    }
    match &b[i..] {
        [] | [b'Z'] => true,
        [sign, h1, h2, b':', m1, m2] if b"+-".contains(sign) => {
            digits(&[*h1, *h2]) && digits(&[*m1, *m2])
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn screens() -> Vec<Screen> {
        crate::config::parse_screens(include_str!("../../examples/screens.toml")).unwrap()
    }

    fn patch(v: Value) -> Result<Checked, String> {
        let p: DisplayPatch = serde_json::from_value(v).map_err(|e| e.to_string())?;
        check(p, &screens())
    }

    fn card(v: Value) -> Result<Card, String> {
        check_card(v, &screens())
    }

    #[test]
    fn every_card_type_accepts_a_good_example() {
        let good = [
            json!({"type": "text", "title": "Note", "body": "One.\n\nTwo."}),
            json!({"type": "list", "items": [{"text": "a", "mark": "done"}, {"text": "b", "detail": "c"}]}),
            json!({"type": "figures", "items": [{"label": "Steps", "value": 8412, "delta": "+4%", "tone": "good"}]}),
            json!({"type": "alerts", "items": [{"level": "crit", "title": "Disk full", "since": "2026-11-14T08:00Z"}]}),
            json!({"type": "calendar", "events": [{"start": "2026-11-14T10:00", "title": "Standup", "where": "Room 2"}]}),
            json!({"type": "goals", "items": [{"name": "Read", "progress": 0.4, "current": 4, "target": 10, "due": "2026-12-31"}]}),
            json!({"type": "chart", "kind": "line", "series": [{"name": "w", "points": [["2026-11-01", 1.5], ["2026-11-02", 2]]}], "highlight": {"at": "2026-11-02"}}),
            json!({"type": "transcript", "lines": 10}),
            json!({"type": "screen", "name": "status"}),
        ];
        for v in good {
            card(v.clone()).unwrap_or_else(|e| panic!("{v}: {e}"));
        }
    }

    #[test]
    fn rejects_unknown_types_fields_and_html_free_text_limits() {
        assert!(card(json!({"type": "html", "body": "<b>x</b>"})).is_err());
        assert!(card(json!({"type": "text", "body": "x", "url": "https://a/"})).is_err());
        assert!(card(json!({"type": "text", "body": "  "})).is_err());
        assert!(card(json!({"type": "text", "body": "a".repeat(2001)})).is_err());
        assert!(card(json!({"type": "list", "items": []})).is_err());
        let many: Vec<Value> = (0..13).map(|i| json!({"text": i.to_string()})).collect();
        assert!(card(json!({"type": "list", "items": many})).is_err());
        assert!(card(json!({"type": "goals", "items": [{"name": "x", "progress": 1.5}]})).is_err());
        assert!(
            card(json!({"type": "calendar", "events": [{"start": "tomorrow", "title": "x"}]}))
                .is_err()
        );
    }

    #[test]
    fn chart_limits_and_highlight_shape() {
        let pts: Vec<Value> = (0..501).map(|i| json!([i, 1])).collect();
        assert!(card(
            json!({"type": "chart", "kind": "bar", "series": [{"name": "a", "points": pts}]})
        )
        .is_err());
        let mixed = json!({"type": "chart", "kind": "bar", "series": [{"name": "a", "points": [[1, 1], ["b", 2]]}]});
        assert!(card(mixed).unwrap_err().contains("not both"));
        let half = json!({"type": "chart", "kind": "line", "series": [{"name": "a", "points": [[1, 1]]}], "highlight": {"from": 1}});
        assert!(card(half).is_err());
        let range = json!({"type": "chart", "kind": "line", "series": [{"name": "a", "points": [[1, 1]]}], "highlight": {"from": 1, "to": 2}});
        assert!(card(range).is_ok());
    }

    #[test]
    fn screen_card_resolves_url_with_allowed_params_only() {
        let c =
            card(json!({"type": "screen", "name": "weather", "params": {"range": "7d"}})).unwrap();
        let Card::Screen(s) = c else { panic!() };
        assert_eq!(
            s.url.as_deref(),
            Some("https://weather.example.com/today?range=7d")
        );
        assert_eq!(s.title.as_deref(), Some("Weather"));
        assert!(
            card(json!({"type": "screen", "name": "weather", "params": {"range": "1000y"}}))
                .is_err()
        );
        assert!(
            card(json!({"type": "screen", "name": "weather", "params": {"src": "x"}})).is_err()
        );
        assert!(card(json!({"type": "screen", "name": "nope"})).is_err());
        let c = card(json!({"type": "screen", "name": "chart", "params": {
            "metric": "category", "category": "Food & Drink", "highlight": "2026-09..2026-10"
        }}))
        .unwrap();
        let Card::Screen(s) = c else { panic!() };
        assert_eq!(
            s.url.as_deref(),
            Some("https://charts.example.com/embed/chart?category=Food%20%26%20Drink&highlight=2026-09..2026-10&metric=category")
        );
        for (k, v) in [
            ("highlight", "2026-13"),
            ("category", "a=b"),
            ("from", "2026-09..2026-10"),
        ] {
            let c = json!({"type": "screen", "name": "chart", "params": {k: v}});
            assert!(card(c).is_err(), "{k}={v}");
        }
        // The agent can't supply the URL itself.
        assert!(
            card(json!({"type": "screen", "name": "status", "url": "https://evil.example/"}))
                .is_err()
        );
    }

    #[test]
    fn errors_name_the_card_index_and_cap_the_count() {
        let e = patch(json!({"content": [{"type": "text", "body": "ok"}, {"type": "nope"}]}))
            .unwrap_err();
        assert!(e.starts_with("content[1]:"), "{e}");
        let four = vec![json!({"type": "text", "body": "x"}); 4];
        assert!(patch(json!({"content": four})).is_err());
    }

    #[test]
    fn status_checks_template_slots_and_kinds() {
        assert!(patch(json!({"status": {"top": {"template": "desk", "items": {"alerts": 2, "next": "Standup 10:00"}}}})).is_ok());
        assert!(patch(json!({"status": {"top": {"template": "fancy"}}})).is_err());
        assert!(
            patch(json!({"status": {"top": {"template": "desk", "items": {"weather": 1}}}}))
                .is_err()
        );
        assert!(patch(
            json!({"status": {"top": {"template": "desk", "items": {"alerts": "two"}}}})
        )
        .is_err());
        let c = patch(json!({"status": {"bottom": null}})).unwrap();
        assert_eq!(c.bottom, Some(None));
        assert_eq!(c.top, None);
    }

    #[test]
    fn presets_follow_content_unless_given() {
        let mut d = Display::default();
        d.apply(patch(json!({"content": [{"type": "text", "body": "x"}]})).unwrap());
        assert_eq!(d.preset, Preset::Split);
        d.apply(patch(json!({"preset": "focus"})).unwrap());
        d.apply(patch(json!({"content": [{"type": "text", "body": "y"}]})).unwrap());
        assert_eq!(d.preset, Preset::Focus, "new cards keep a non-chat preset");
        d.apply(patch(json!({"content": []})).unwrap());
        assert_eq!(d.preset, Preset::Chat);
        d.apply(
            patch(json!({"preset": "chat", "content": [{"type": "text", "body": "z"}]})).unwrap(),
        );
        assert_eq!(d.preset, Preset::Chat, "an explicit preset wins");
    }

    #[test]
    fn avatar_only_patch_leaves_the_display_alone() {
        let c = patch(json!({"avatar": "thinking"})).unwrap();
        assert!(!c.touches_display());
        assert_eq!(c.avatar, Some(Avatar::Thinking));
        assert!(patch(json!({"avatar": "dancing"})).is_err());
    }

    #[test]
    fn iso_times() {
        for ok in [
            "2026-11-14",
            "2026-11-14T09:30",
            "2026-11-14T09:30:05",
            "2026-11-14T09:30:05.123Z",
            "2026-11-14T09:30-08:00",
        ] {
            assert!(is_time(ok), "{ok}");
        }
        for bad in [
            "2026-1-14",
            "2026-11-14 09:30",
            "2026-11-14T9:30",
            "2026-11-14T09:30+8",
            "now",
        ] {
            assert!(!is_time(bad), "{bad}");
        }
    }
}
