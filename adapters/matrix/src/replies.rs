//! Hold the agent's replies until they settle, so a reply streamed as a
//! message plus a run of edits is spoken once, in its final form.
//!
//! A reply is due when `settle` passes with no edit to it. An edit that
//! arrives after the reply was spoken and only extends it (the agent
//! paused mid-stream) queues just the new tail; any other late edit is
//! dropped, since the words are already out.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// How many spoken replies to remember for late edits.
const REMEMBER: usize = 32;

#[derive(Debug)]
struct Pending {
    id: String,
    body: String,
    /// Already spoken (a late edit's original); only the rest is said.
    spoken: String,
    due: Instant,
}

#[derive(Debug)]
pub struct Replies {
    settle: Duration,
    pending: Vec<Pending>,
    spoken: VecDeque<(String, String)>,
}

impl Replies {
    pub fn new(settle: Duration) -> Self {
        Replies {
            settle,
            pending: Vec::new(),
            spoken: VecDeque::new(),
        }
    }

    /// A new message from the agent.
    pub fn message(&mut self, id: &str, body: &str, now: Instant) {
        if self.pending.iter().any(|p| p.id == id) || self.spoken.iter().any(|(s, _)| s == id) {
            return; // a repeat (e.g. a sync retried)
        }
        self.pending.push(Pending {
            id: id.to_string(),
            body: body.to_string(),
            spoken: String::new(),
            due: now + self.settle,
        });
    }

    /// An edit replacing message `target`'s text with `body`.
    pub fn edit(&mut self, target: &str, body: &str, now: Instant) {
        if let Some(p) = self.pending.iter_mut().find(|p| p.id == target) {
            p.body = body.to_string();
            p.due = now + self.settle;
            return;
        }
        let Some((_, said)) = self.spoken.iter().find(|(id, _)| id == target) else {
            return; // a message from before the adapter started
        };
        if body.len() > said.len() && body.starts_with(said.as_str()) {
            let spoken = said.clone();
            self.pending.push(Pending {
                id: target.to_string(),
                body: body.to_string(),
                spoken,
                due: now + self.settle,
            });
        }
    }

    /// When the next reply falls due, if any is waiting.
    pub fn next_due(&self) -> Option<Instant> {
        self.pending.iter().map(|p| p.due).min()
    }

    /// Whether any reply is still settling.
    pub fn is_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    /// Take the replies due at `now`, in arrival order: each the text to
    /// say (for a late edit, only the new tail).
    pub fn take_due(&mut self, now: Instant) -> Vec<String> {
        let mut out = Vec::new();
        let mut i = 0;
        while i < self.pending.len() {
            if self.pending[i].due > now {
                i += 1;
                continue;
            }
            let p = self.pending.remove(i);
            out.push(p.body[p.spoken.len()..].to_string());
            self.spoken.retain(|(id, _)| *id != p.id);
            self.spoken.push_back((p.id, p.body));
            if self.spoken.len() > REMEMBER {
                self.spoken.pop_front();
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SETTLE: Duration = Duration::from_millis(1500);

    fn ms(t0: Instant, n: u64) -> Instant {
        t0 + Duration::from_millis(n)
    }

    #[test]
    fn a_streamed_reply_is_spoken_once_settled() {
        let t0 = Instant::now();
        let mut r = Replies::new(SETTLE);
        r.message("$1", "Two", t0);
        r.edit("$1", "Two new", ms(t0, 500));
        r.edit("$1", "Two new mail.", ms(t0, 1000));
        assert_eq!(r.next_due(), Some(ms(t0, 2500)));
        assert!(r.take_due(ms(t0, 2499)).is_empty());
        assert_eq!(r.take_due(ms(t0, 2500)), vec!["Two new mail."]);
        assert!(!r.is_pending());
        assert_eq!(r.next_due(), None);
    }

    #[test]
    fn replies_come_out_in_arrival_order() {
        let t0 = Instant::now();
        let mut r = Replies::new(SETTLE);
        r.message("$1", "first", t0);
        r.message("$2", "second", ms(t0, 10));
        r.edit("$1", "first, edited", ms(t0, 20));
        assert_eq!(r.take_due(ms(t0, 5000)), vec!["first, edited", "second"]);
    }

    #[test]
    fn a_late_extending_edit_says_only_the_tail() {
        let t0 = Instant::now();
        let mut r = Replies::new(SETTLE);
        r.message("$1", "Checking your mail.", t0);
        assert_eq!(r.take_due(ms(t0, 1500)).len(), 1);
        r.edit("$1", "Checking your mail. Two new.", ms(t0, 4000));
        assert_eq!(r.take_due(ms(t0, 5500)), vec![" Two new."]);
        // A rewrite of something already said is dropped.
        r.edit("$1", "Actually, none.", ms(t0, 6000));
        assert!(!r.is_pending());
    }

    #[test]
    fn ignores_repeats_and_edits_of_unknown_messages() {
        let t0 = Instant::now();
        let mut r = Replies::new(SETTLE);
        r.edit("$old", "from before startup", t0);
        assert!(!r.is_pending());
        r.message("$1", "hi", t0);
        r.message("$1", "hi", t0);
        assert_eq!(r.take_due(ms(t0, 2000)), vec!["hi"]);
        r.message("$1", "hi", ms(t0, 3000));
        assert!(!r.is_pending());
    }
}
