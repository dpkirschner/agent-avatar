//! Parse a Matrix `/sync` response into what the adapter acts on: the
//! agent's messages, its edits of them, and its typing state, for one
//! room. Pure; the HTTP lives in `homeserver.rs`.

use serde_json::Value;

/// Something the agent did in the room.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Incoming {
    /// A new `m.text` or `m.notice` message.
    Message { event_id: String, body: String },
    /// An `m.replace` edit: `target` is the original message's id, `body`
    /// the full new text (`m.new_content`).
    Edit { target: String, body: String },
    /// The agent started (`true`) or stopped typing.
    Typing(bool),
}

/// One `/sync` response, reduced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncPage {
    pub next_batch: String,
    pub items: Vec<Incoming>,
}

/// Reduce a `/sync` response to the agent's activity in `room`.
/// Everything from other senders, other rooms, and other event types is
/// dropped, and so are messages sent before `min_ts` (milliseconds since
/// the epoch), so a sync after a long outage doesn't read out stale
/// replies. A response without `next_batch` is an error.
pub fn parse_sync(resp: &Value, room: &str, agent: &str, min_ts: u64) -> Result<SyncPage, String> {
    let next_batch = resp
        .get("next_batch")
        .and_then(Value::as_str)
        .ok_or("sync response has no next_batch")?
        .to_string();
    let mut items = Vec::new();
    let joined = &resp["rooms"]["join"][room];
    if let Some(events) = joined["timeline"]["events"].as_array() {
        items.extend(
            events
                .iter()
                .filter(|ev| ev["origin_server_ts"].as_u64().unwrap_or(u64::MAX) >= min_ts)
                .filter_map(|ev| message(ev, agent)),
        );
    }
    if let Some(events) = joined["ephemeral"]["events"].as_array() {
        // The last m.typing in the batch is the current state.
        if let Some(typing) = events
            .iter()
            .filter(|ev| ev["type"] == "m.typing")
            .filter_map(|ev| ev["content"]["user_ids"].as_array())
            .next_back()
        {
            items.push(Incoming::Typing(
                typing.iter().any(|u| u.as_str() == Some(agent)),
            ));
        }
    }
    Ok(SyncPage { next_batch, items })
}

fn message(ev: &Value, agent: &str) -> Option<Incoming> {
    if ev["type"] != "m.room.message" || ev["sender"].as_str() != Some(agent) {
        return None;
    }
    let content = &ev["content"];
    let rel = &content["m.relates_to"];
    if rel["rel_type"] == "m.replace" {
        let target = rel["event_id"].as_str()?.to_string();
        let body = speakable_body(&content["m.new_content"])?;
        return Some(Incoming::Edit { target, body });
    }
    Some(Incoming::Message {
        event_id: ev["event_id"].as_str()?.to_string(),
        body: speakable_body(content)?,
    })
}

/// The body of a text or notice message; images, files and redacted
/// events (no body) are not spoken.
fn speakable_body(content: &Value) -> Option<String> {
    match content["msgtype"].as_str()? {
        "m.text" | "m.notice" => content["body"].as_str().map(str::to_string),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const ROOM: &str = "!room:example.org";
    const AGENT: &str = "@agent:example.org";

    fn page(timeline: Value, ephemeral: Value) -> Value {
        json!({
            "next_batch": "s2",
            "rooms": {"join": {
                ROOM: {"timeline": {"events": timeline}, "ephemeral": {"events": ephemeral}},
                "!other:example.org": {"timeline": {"events": [
                    {"type": "m.room.message", "sender": AGENT, "event_id": "$x",
                     "content": {"msgtype": "m.text", "body": "elsewhere"}}
                ]}}
            }}
        })
    }

    #[test]
    fn keeps_the_agents_messages_and_edits_in_the_room() {
        let resp = page(
            json!([
                {"type": "m.room.message", "sender": "@tablet:example.org", "event_id": "$0",
                 "content": {"msgtype": "m.text", "body": "what's new"}},
                {"type": "m.room.message", "sender": AGENT, "event_id": "$1",
                 "content": {"msgtype": "m.text", "body": "Two"}},
                {"type": "m.room.message", "sender": AGENT, "event_id": "$2",
                 "content": {"msgtype": "m.text", "body": "* Two new mail",
                   "m.new_content": {"msgtype": "m.text", "body": "Two new mail"},
                   "m.relates_to": {"rel_type": "m.replace", "event_id": "$1"}}},
                {"type": "m.room.message", "sender": AGENT, "event_id": "$3",
                 "content": {"msgtype": "m.image", "body": "chart.png"}},
                {"type": "m.room.message", "sender": AGENT, "event_id": "$4", "content": {}},
                {"type": "m.reaction", "sender": AGENT, "event_id": "$5", "content": {}},
                {"type": "m.room.message", "sender": AGENT, "event_id": "$6",
                 "content": {"msgtype": "m.notice", "body": "Working on it"}}
            ]),
            json!([]),
        );
        let p = parse_sync(&resp, ROOM, AGENT, 0).unwrap();
        assert_eq!(p.next_batch, "s2");
        assert_eq!(
            p.items,
            vec![
                Incoming::Message {
                    event_id: "$1".into(),
                    body: "Two".into()
                },
                Incoming::Edit {
                    target: "$1".into(),
                    body: "Two new mail".into()
                },
                Incoming::Message {
                    event_id: "$6".into(),
                    body: "Working on it".into()
                },
            ]
        );
    }

    #[test]
    fn reports_the_agents_typing_state() {
        let typing = |users: Value| {
            let resp = page(
                json!([]),
                json!([{"type": "m.typing", "content": {"user_ids": users}}]),
            );
            parse_sync(&resp, ROOM, AGENT, 0).unwrap().items
        };
        assert_eq!(typing(json!([AGENT])), vec![Incoming::Typing(true)]);
        assert_eq!(
            typing(json!(["@someone:example.org"])),
            vec![Incoming::Typing(false)]
        );
        let quiet = parse_sync(&page(json!([]), json!([])), ROOM, AGENT, 0).unwrap();
        assert!(quiet.items.is_empty());
    }

    #[test]
    fn drops_messages_older_than_min_ts() {
        let resp = page(
            json!([
                {"type": "m.room.message", "sender": AGENT, "event_id": "$old",
                 "origin_server_ts": 1000, "content": {"msgtype": "m.text", "body": "stale"}},
                {"type": "m.room.message", "sender": AGENT, "event_id": "$new",
                 "origin_server_ts": 5000, "content": {"msgtype": "m.text", "body": "fresh"}}
            ]),
            json!([]),
        );
        let items = parse_sync(&resp, ROOM, AGENT, 2000).unwrap().items;
        assert_eq!(
            items,
            vec![Incoming::Message {
                event_id: "$new".into(),
                body: "fresh".into()
            }]
        );
    }

    #[test]
    fn needs_next_batch_and_tolerates_missing_rooms() {
        assert!(parse_sync(&json!({}), ROOM, AGENT, 0).is_err());
        let p = parse_sync(&json!({"next_batch": "s9"}), ROOM, AGENT, 0).unwrap();
        assert!(p.items.is_empty());
    }
}
