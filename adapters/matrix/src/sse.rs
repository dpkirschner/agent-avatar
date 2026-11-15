//! A minimal Server-Sent Events parser for the display server's
//! `/heard/events` stream: feed it bytes as they arrive, take out whole
//! events.

/// One dispatched event: its `event:` name (`message` when absent) and
/// its `data:` lines joined with newlines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseEvent {
    pub event: String,
    pub data: String,
}

/// Incremental parser. Handles `\n`, `\r\n` and `\r` line ends, comment
/// lines, and chunks that split a line or a UTF-8 character.
#[derive(Debug, Default)]
pub struct SseParser {
    buf: Vec<u8>,
    event: String,
    data: Vec<String>,
}

impl SseParser {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a chunk and return the events it completed.
    pub fn feed(&mut self, chunk: &[u8]) -> Vec<SseEvent> {
        self.buf.extend_from_slice(chunk);
        let mut out = Vec::new();
        while let Some(pos) = self.buf.iter().position(|&b| b == b'\n' || b == b'\r') {
            // A lone `\r` at the end of the buffer may be the first half
            // of `\r\n`; wait for the next chunk.
            if self.buf[pos] == b'\r' && pos + 1 == self.buf.len() {
                break;
            }
            let skip = if self.buf[pos] == b'\r' && self.buf[pos + 1] == b'\n' {
                2
            } else {
                1
            };
            let line: Vec<u8> = self.buf.drain(..pos + skip).take(pos).collect();
            let line = String::from_utf8_lossy(&line);
            if let Some(ev) = self.line(&line) {
                out.push(ev);
            }
        }
        out
    }

    fn line(&mut self, line: &str) -> Option<SseEvent> {
        if line.is_empty() {
            let ev = SseEvent {
                event: std::mem::take(&mut self.event),
                data: std::mem::take(&mut self.data).join("\n"),
            };
            if ev.data.is_empty() && ev.event.is_empty() {
                return None;
            }
            return Some(SseEvent {
                event: if ev.event.is_empty() {
                    "message".into()
                } else {
                    ev.event
                },
                data: ev.data,
            });
        }
        if line.starts_with(':') {
            return None;
        }
        let (field, value) = match line.split_once(':') {
            Some((f, v)) => (f, v.strip_prefix(' ').unwrap_or(v)),
            None => (line, ""),
        };
        match field {
            "event" => self.event = value.to_string(),
            "data" => self.data.push(value.to_string()),
            _ => {} // id, retry: not needed here
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_events_split_across_chunks() {
        let mut p = SseParser::new();
        assert!(p.feed(b": keep-alive\n\nevent: hea").is_empty());
        assert!(p.feed(b"rd\r\ndata: {\"text\":\"h").is_empty());
        let evs = p.feed(b"i\"}\r\n\r\nretry: 2000\ndata: a\ndata: b\n\n");
        assert_eq!(
            evs,
            vec![
                SseEvent {
                    event: "heard".into(),
                    data: r#"{"text":"hi"}"#.into()
                },
                SseEvent {
                    event: "message".into(),
                    data: "a\nb".into()
                },
            ]
        );
    }

    #[test]
    fn waits_for_the_lf_after_a_trailing_cr() {
        let mut p = SseParser::new();
        assert!(p.feed(b"data: x\r").is_empty());
        assert!(p.feed(b"\n\r").is_empty());
        assert_eq!(p.feed(b"\n").len(), 1);
    }

    #[test]
    fn keeps_multibyte_characters_split_between_chunks() {
        let mut p = SseParser::new();
        let text = "data: café\n\n".as_bytes();
        let (a, b) = text.split_at(10); // inside the é
        assert!(p.feed(a).is_empty());
        assert_eq!(p.feed(b)[0].data, "café");
    }
}
