//! Turn the agent's messages and edits into lines to say, as early as
//! they are safe to say.
//!
//! An agent that streams a reply (MSC4357) posts a draft marked live,
//! edits it while live, and ends with one edit that drops the mark: that
//! is the final text. A message without the mark is final as sent.
//!
//! - While live, complete sentences are spoken as they stabilize: a
//!   sentence is safe once more text follows it. Only text that extends
//!   what was already said is spoken. A draft that no longer starts with
//!   it was replaced, and nothing more is said until the final.
//! - The final says what of it hasn't been said: the text after the
//!   longest sentence-aligned prefix it shares with what was said.
//! - Tool progress ("Working" plus backticked tool lines) is never said.
//! - A live draft with no edit for [`STUCK`] is taken as final.
//! - An edit after the final that only extends it says the new tail; any
//!   other late edit is dropped, since the words are already out.
//!
//! Text is compared after [`prepare`]: progress dropped, markdown
//! cleaned for speech, whitespace collapsed.

use crate::speech;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// How long a live draft may go without an edit before it is final.
pub const STUCK: Duration = Duration::from_secs(30);

/// How many finished replies to remember for late edits.
const REMEMBER: usize = 32;

#[derive(Debug)]
struct Live {
    id: String,
    /// The newest draft, prepared.
    text: String,
    /// What has been said: a prefix of `text` unless `replaced`.
    said: String,
    replaced: bool,
    last_edit: Instant,
}

#[derive(Debug, Default)]
pub struct Replies {
    live: Vec<Live>,
    /// Finished replies: id and the prepared text as said.
    done: VecDeque<(String, String)>,
}

impl Replies {
    pub fn new() -> Self {
        Self::default()
    }

    /// A new message from the agent; returns the lines to say now.
    pub fn message(&mut self, id: &str, body: &str, live: bool, now: Instant) -> Vec<String> {
        if self.live.iter().any(|l| l.id == id) || self.done.iter().any(|(d, _)| d == id) {
            return Vec::new(); // a repeat (e.g. a sync retried)
        }
        let text = prepare(body);
        if !live {
            let out = line(&text);
            self.finish(id, text);
            return out;
        }
        let mut l = Live::new(id, String::new(), now);
        let out = l.draft(text);
        self.live.push(l);
        out
    }

    /// An edit replacing message `target`'s text with `body`; returns the
    /// lines to say now.
    pub fn edit(&mut self, target: &str, body: &str, live: bool, now: Instant) -> Vec<String> {
        let text = prepare(body);
        if let Some(i) = self.live.iter().position(|l| l.id == target) {
            if live {
                self.live[i].last_edit = now;
                return self.live[i].draft(text);
            }
            let l = self.live.remove(i);
            let out = line(unsaid(&text, &l.said));
            self.finish(target, text);
            return out;
        }
        let Some(i) = self.done.iter().position(|(id, _)| id == target) else {
            return Vec::new(); // a message from before the adapter started
        };
        let said = &self.done[i].1;
        if text.len() <= said.len() || !text.starts_with(said.as_str()) {
            return Vec::new();
        }
        let (_, said) = self.done.remove(i).expect("index is in range");
        if live {
            // The agent went back to streaming the same message.
            let mut l = Live::new(target, said, now);
            let out = l.draft(text);
            self.live.push(l);
            return out;
        }
        let out = line(&text[said.len()..]);
        self.finish(target, text);
        out
    }

    /// When the next live draft goes stuck, if any is open.
    pub fn next_due(&self) -> Option<Instant> {
        self.live.iter().map(|l| l.last_edit + STUCK).min()
    }

    /// Whether any reply is still streaming.
    pub fn is_pending(&self) -> bool {
        !self.live.is_empty()
    }

    /// Finish the drafts stuck at `now` and return what's left to say.
    pub fn take_due(&mut self, now: Instant) -> Vec<String> {
        let mut out = Vec::new();
        let mut i = 0;
        while i < self.live.len() {
            if self.live[i].last_edit + STUCK > now {
                i += 1;
                continue;
            }
            let l = self.live.remove(i);
            out.extend(line(unsaid(&l.text, &l.said)));
            self.finish(&l.id, l.text);
        }
        out
    }

    fn finish(&mut self, id: &str, text: String) {
        self.done.retain(|(d, _)| d != id);
        self.done.push_back((id.to_string(), text));
        if self.done.len() > REMEMBER {
            self.done.pop_front();
        }
    }
}

impl Live {
    fn new(id: &str, said: String, now: Instant) -> Self {
        Live {
            id: id.to_string(),
            text: said.clone(),
            said,
            replaced: false,
            last_edit: now,
        }
    }

    /// Take a new live draft; return the newly complete sentences.
    fn draft(&mut self, text: String) -> Vec<String> {
        self.text = text;
        if self.replaced {
            return Vec::new();
        }
        if !self.text.starts_with(self.said.as_str()) {
            self.replaced = true;
            return Vec::new();
        }
        let end = complete_prefix(&self.text);
        if end <= self.said.len() {
            return Vec::new();
        }
        let out = line(&self.text[self.said.len()..end]);
        self.said = self.text[..end].to_string();
        out
    }
}

/// `text` as a line to say (trimmed, without the punctuation a tail can
/// start with), or nothing when that leaves it blank.
fn line(text: &str) -> Vec<String> {
    let t = text
        .trim()
        .trim_start_matches(['.', ',', ';', ':', '!', '?'])
        .trim();
    if t.is_empty() {
        Vec::new()
    } else {
        vec![speech::fit(t)]
    }
}

/// The part of `fin` not yet said, given `said`: everything after the
/// longest prefix the two share that ends on a sentence boundary.
fn unsaid<'a>(fin: &'a str, said: &str) -> &'a str {
    if let Some(rest) = fin.strip_prefix(said) {
        return rest;
    }
    let common = fin
        .char_indices()
        .zip(said.chars())
        .find(|((_, a), b)| a != b)
        .map_or(fin.len().min(said.len()), |((i, _), _)| i);
    let at = boundaries(fin)
        .take_while(|&b| b <= common)
        .last()
        .unwrap_or(0);
    &fin[at..]
}

/// Byte offsets where a sentence or line ends in `text`: after `.`, `!`
/// or `?` followed by whitespace, and at each line break.
fn boundaries(text: &str) -> impl Iterator<Item = usize> + '_ {
    let mut chars = text.char_indices().peekable();
    std::iter::from_fn(move || {
        while let Some((i, c)) = chars.next() {
            if c == '\n' {
                return Some(i);
            }
            if matches!(c, '.' | '!' | '?') && chars.peek().is_some_and(|(_, n)| n.is_whitespace())
            {
                return Some(i + 1);
            }
        }
        None
    })
}

/// The end of the last sentence in `text` that more text follows: what
/// of a live draft is safe to say.
fn complete_prefix(text: &str) -> usize {
    boundaries(text)
        .filter(|&b| !text[b..].trim().is_empty())
        .last()
        .unwrap_or(0)
}

/// A message body as compared and spoken: the leading tool progress
/// dropped, cleaned for speech, whitespace collapsed.
pub fn prepare(body: &str) -> String {
    speech::clean(strip_progress(body))
}

/// Drop a leading progress block: a short label line ("Working"), a
/// blank line, then one or more lines that are each one backticked span
/// (`` `🛠️ Exec: running` ``), with blank lines between them allowed.
/// Whatever follows is the answer. A body that doesn't open that way is
/// returned whole.
fn strip_progress(body: &str) -> &str {
    let mut at = 0;
    let mut tools = 0;
    for (n, l) in body.split_inclusive('\n').enumerate() {
        let t = l.trim();
        let ok = match n {
            0 => {
                let label = t.trim_end_matches(['.', '…']);
                !label.is_empty()
                    && label.split_whitespace().count() <= 3
                    && !label.contains(['.', '!', '?', '`', ':'])
            }
            1 => t.is_empty(),
            _ if t.is_empty() => true,
            _ => {
                let tool = t.len() >= 2
                    && t.starts_with('`')
                    && t.ends_with('`')
                    && !t[1..t.len() - 1].contains('`');
                if !tool && tools > 0 {
                    break; // the answer starts here
                }
                tools += usize::from(tool);
                tool
            }
        };
        if !ok {
            return body;
        }
        at += l.len();
    }
    if tools == 0 {
        body
    } else {
        &body[at..]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(t0: Instant, n: u64) -> Instant {
        t0 + Duration::from_secs(n)
    }

    const PROGRESS: &str = "Working\n\n`🛠️ Exec: running`";

    #[test]
    fn progress_then_answer_then_final() {
        let t0 = Instant::now();
        let mut r = Replies::new();
        assert!(r.message("$1", PROGRESS, true, t0).is_empty());
        let two = format!("{PROGRESS}\n`🛠️ Read: done`");
        assert!(r.edit("$1", &two, true, s(t0, 5)).is_empty());
        // Progress replaced by the start of the answer: nothing said yet.
        assert!(r.edit("$1", "You have two", true, s(t0, 6)).is_empty());
        assert!(r.is_pending());
        assert_eq!(
            r.edit("$1", "You have two new mails.", false, s(t0, 7)),
            vec!["You have two new mails."]
        );
        assert!(!r.is_pending());
        assert_eq!(r.next_due(), None);
    }

    #[test]
    fn a_non_streamed_message_is_said_at_once() {
        let t0 = Instant::now();
        let mut r = Replies::new();
        assert_eq!(
            r.message("$1", "**Done.** All set", false, t0),
            vec!["Done. All set"]
        );
        assert!(!r.is_pending());
        // A repeat (sync retried) says nothing.
        assert!(r.message("$1", "**Done.** All set", false, t0).is_empty());
        // A late edit that extends it says the tail; a rewrite says nothing.
        assert_eq!(
            r.edit("$1", "**Done.** All set. Bye.", false, s(t0, 1)),
            vec!["Bye."]
        );
        assert!(r.edit("$1", "Actually, no.", false, s(t0, 2)).is_empty());
    }

    #[test]
    fn a_stream_is_said_sentence_by_sentence() {
        let t0 = Instant::now();
        let mut r = Replies::new();
        assert!(r.message("$1", "Two new", true, t0).is_empty());
        // The sentence end alone isn't enough: more text must follow.
        assert!(r.edit("$1", "Two new mails.", true, s(t0, 1)).is_empty());
        assert_eq!(
            r.edit("$1", "Two new mails. The first", true, s(t0, 2)),
            vec!["Two new mails."]
        );
        assert!(r
            .edit("$1", "Two new mails. The first is from Sam", true, s(t0, 3))
            .is_empty());
        assert_eq!(
            r.edit(
                "$1",
                "Two new mails. The first is from Sam! Next:\n- rent",
                true,
                s(t0, 4)
            ),
            vec!["The first is from Sam! Next:"]
        );
        assert_eq!(
            r.edit(
                "$1",
                "Two new mails. The first is from Sam! Next:\n- rent\n- tax",
                false,
                s(t0, 5)
            ),
            vec!["rent\ntax"]
        );
    }

    #[test]
    fn a_replaced_draft_stops_and_the_final_says_what_differs() {
        let t0 = Instant::now();
        let mut r = Replies::new();
        assert_eq!(
            r.message("$1", "Let me look. One sec", true, t0),
            vec!["Let me look."]
        );
        // The draft is rewritten: nothing more until the final.
        assert!(r
            .edit("$1", "Found it. Two mails. Th", true, s(t0, 1))
            .is_empty());
        assert!(r
            .edit("$1", "Found it. Two mails. The first", true, s(t0, 2))
            .is_empty());
        assert_eq!(
            r.edit("$1", "Found it. Two mails.", false, s(t0, 3)),
            vec!["Found it. Two mails."]
        );

        // A final that keeps the start of what was said says only the rest.
        let mut r = Replies::new();
        assert_eq!(
            r.message("$2", "One. Two. Thr", true, t0),
            vec!["One. Two."]
        );
        assert!(r.edit("$2", "One. Too", true, s(t0, 1)).is_empty());
        assert_eq!(r.edit("$2", "One. Three.", false, s(t0, 2)), vec!["Three."]);
    }

    #[test]
    fn a_stuck_draft_is_final_after_thirty_seconds() {
        let t0 = Instant::now();
        let mut r = Replies::new();
        assert_eq!(
            r.message("$1", "Sure. Checking now", true, t0),
            vec!["Sure."]
        );
        r.edit("$1", "Sure. Checking now, hold on", true, s(t0, 10));
        assert_eq!(r.next_due(), Some(s(t0, 40)));
        assert!(r.take_due(s(t0, 39)).is_empty());
        assert_eq!(r.take_due(s(t0, 40)), vec!["Checking now, hold on"]);
        assert!(!r.is_pending());

        // A stuck progress-only draft says nothing.
        let mut r = Replies::new();
        r.message("$2", PROGRESS, true, t0);
        assert!(r.take_due(s(t0, 30)).is_empty());
        assert!(!r.is_pending());
    }

    #[test]
    fn edits_of_unknown_messages_are_ignored() {
        let mut r = Replies::new();
        assert!(r
            .edit("$old", "from before startup", false, Instant::now())
            .is_empty());
        assert!(!r.is_pending());
    }

    #[test]
    fn strips_only_a_structural_progress_block() {
        assert_eq!(strip_progress(PROGRESS), "");
        assert_eq!(
            strip_progress("Working…\n\n`🛠️ Exec: ls`\n\n`📖 Read: a.md`\n\nHere you go."),
            "Here you go."
        );
        for body in [
            "Working\n\nOn it.",            // no tool lines
            "Sure, working on it.\n\n`ls`", // a sentence, not a label
            "Working\n`ls`",                // no blank line
            "Run this:\n\n`ls -la`",        // a colon: an answer
            "`ls`",
            "",
        ] {
            assert_eq!(strip_progress(body), body, "{body:?}");
        }
    }

    #[test]
    fn complete_prefix_needs_text_after_the_end() {
        assert_eq!(complete_prefix("Hi. There"), 3);
        assert_eq!(complete_prefix("Hi. There."), 3);
        assert_eq!(complete_prefix("Hi. "), 0);
        assert_eq!(complete_prefix("v1.2 is out"), 0);
        assert_eq!(complete_prefix("Title\nbody"), 5);
        assert_eq!(complete_prefix("Ünïcode! ok"), "Ünïcode!".len());
    }
}
