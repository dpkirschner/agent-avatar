//! Turn a chat message into something worth reading aloud: drop the
//! reply quote and code blocks, unwrap markdown, shorten URLs, and fit
//! the display server's length limit.

/// Longest line `POST /say` accepts, in characters.
pub const MAX_SAY_CHARS: usize = 2000;

/// Clean `body` for speech. Returns `None` when nothing speakable is
/// left (an empty message, or only code).
pub fn for_speech(body: &str) -> Option<String> {
    let mut lines = Vec::new();
    let mut in_code = false;
    let mut quote_prefix = body.trim_start().starts_with("> <@");
    for raw in body.lines() {
        let line = raw.trim();
        if line.starts_with("```") || line.starts_with("~~~") {
            in_code = !in_code;
            continue;
        }
        if in_code {
            continue;
        }
        // A reply fallback (`> <@user> ...`, then more `>` lines) leads
        // the body; a quote that doesn't start that way is kept.
        if quote_prefix {
            if line.starts_with('>') {
                continue;
            }
            quote_prefix = false;
        }
        let line = strip_block_markers(line);
        let line = unwrap_inline(line);
        if !line.is_empty() {
            lines.push(line);
        }
    }
    let text = lines.join("\n");
    let text = text.trim();
    if text.is_empty() {
        None
    } else {
        Some(truncate(text, MAX_SAY_CHARS))
    }
}

fn strip_block_markers(line: &str) -> &str {
    let mut l = line;
    loop {
        let before = l;
        l = l.trim_start_matches('#').trim_start();
        l = l.strip_prefix('>').unwrap_or(l).trim_start();
        for bullet in ["- ", "* ", "+ "] {
            l = l.strip_prefix(bullet).unwrap_or(l);
        }
        if l == before {
            return l;
        }
    }
}

/// Unwrap `[text](url)` to `text`, replace bare URLs with "link", and
/// drop `**`, `__`, `~~`, backticks and `*` emphasis markers.
fn unwrap_inline(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix('[') {
            if let Some(close) = after.find("](") {
                if let Some(end) = after[close + 2..].find(')') {
                    out.push_str(&after[..close]);
                    rest = &after[close + 2 + end + 1..];
                    continue;
                }
            }
        }
        if rest.starts_with("https://") || rest.starts_with("http://") {
            let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
            out.push_str("link");
            rest = &rest[end..];
            continue;
        }
        let mut chars = rest.chars();
        let c = chars.next().expect("rest is not empty");
        let skip = matches!(c, '`' | '*')
            || ((c == '_' || c == '~') && rest[c.len_utf8()..].starts_with(c));
        if skip {
            let n = if c == '_' || c == '~' { 2 } else { 1 };
            rest = &rest[n * c.len_utf8()..];
            continue;
        }
        out.push(c);
        rest = chars.as_str();
    }
    out.trim().to_string()
}

/// Cut `text` to at most `max` characters, at the last sentence end or
/// space before the limit when there is one.
fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let cut: String = text.chars().take(max - 1).collect();
    let at = cut
        .rfind(['.', '!', '?'])
        .map(|i| i + 1)
        .filter(|&i| i > max / 2)
        .or_else(|| cut.rfind(char::is_whitespace))
        .unwrap_or(cut.len());
    format!("{}…", cut[..at].trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unwraps_markdown() {
        let body = "# Today\n\n- **Two** new mail, see [the inbox](https://mail.example.com/)\n- run `make check`\n\nMore at https://example.com/x?y=1 now.";
        assert_eq!(
            for_speech(body).unwrap(),
            "Today\nTwo new mail, see the inbox\nrun make check\nMore at link now."
        );
    }

    #[test]
    fn drops_code_blocks_and_reply_fallback() {
        let body = "> <@tablet:example.org> what's up\n> second line\n\nNothing much.\n```\nls -la\n```\nDone.";
        assert_eq!(for_speech(body).unwrap(), "Nothing much.\nDone.");
        assert_eq!(for_speech("```\nonly code\n```"), None);
        assert_eq!(for_speech("  \n "), None);
    }

    #[test]
    fn keeps_snake_case_and_single_tildes() {
        assert_eq!(
            for_speech("set my_var to ~5 ~~not~~").unwrap(),
            "set my_var to ~5 not"
        );
    }

    #[test]
    fn truncates_at_a_sentence_end() {
        let long = "One sentence here. ".repeat(200);
        let out = for_speech(&long).unwrap();
        assert!(out.chars().count() <= MAX_SAY_CHARS);
        assert!(out.ends_with(".…"), "{}", &out[out.len() - 20..]);
    }
}
