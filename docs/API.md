# API

The display server's HTTP interface. JSON bodies use snake_case
fields; errors come back as `{"error": "..."}`.

## Auth

With a command token configured (`AVATAR_TOKEN_FILE`, a 0600 file, or
`AVATAR_TOKEN`), the commands, `POST /heard` and the heard stream need
`Authorization: Bearer <token>`; without it they answer `401`, before
the body is read. `POST /heard` is guarded because its text reaches the
agent as if the owner said it. The page sends the token once it is
pasted into its settings (it asks on the first start tap). With no
token everything is open to whatever can reach the port, and the
server logs a warning at startup.

The page's own routes are always open: the page, its event stream,
`/state` and `/screens`. Keep the server behind a
private network (loopback plus a tailnet-only proxy is the intended
setup).

## Page and health (open)

| Method | Path       | Response |
|--------|------------|----------|
| GET    | `/`        | The kiosk page (`/app.js`, `/app.css` and `/avatars/<face>/avatar.{js,css}` alongside). Its CSP frames only the configured screens' origins. |
| GET    | `/healthz` | `200 ok` |
| GET    | `/version` | `{"version": "v2026.11.0"}` (the release tag, or the crate version in a dev build) |
| GET    | `/status`  | `{"pages": 1, "page_seen_secs": 0, "adapters": 1}`: pages and adapters (`/heard/events` listeners) connected now, and seconds since a page was last connected (0 while one is; seconds since the server started if none has been). For monitoring. |
| GET    | `/state`   | `{"preset", "content", "status", "avatar", "last_say"}`: the whole display (as in the `display` event), the avatar state, and the last line said or `null` |
| GET    | `/screens` | `[{"name": "weather", "title": "Weather", "params": {"range": ["today", "7d"], "day": "date"}}]`: names, titles and allowed params, never URLs |

## Commands (agent → server, token)

| Method | Path       | Body | Effect |
|--------|------------|------|--------|
| POST   | `/display` | `{"preset", "avatar", "content", "status"}`, all optional | Change what the page shows. Returns the whole state, as `/state`. |
| POST   | `/say`     | `{"text": "Done, three new mail."}` | Speak a line (trimmed, 1–2000 characters, else `400`). Returns `{"id", "text", "audio"}`. The page shows `talking` while it plays; the avatar state becomes `idle`. |

### `POST /display`

Each field sent replaces that part of the display; fields left out
keep what is there. Everything is checked before anything changes: a
bad field answers `422` with an error naming it (`content[1]: ...`),
and the display stays as it was.

```json
{
  "preset":  "split",
  "avatar":  "thinking",
  "content": [{"type": "text", "body": "Hello."}],
  "status":  {"top": {"template": "desk", "items": {"mail": 3}}, "bottom": null}
}
```

- `preset`: `chat` (avatar wide, content narrow), `split` (about half
  each) or `focus` (avatar a narrow column, content dominant). The page
  sets the geometry; the agent never sends it.
- `avatar`: `idle`, `listening`, `thinking` or `talking`.
  `{"avatar": "thinking"}` alone is the cheap call for "typing".
- `content`: up to 3 cards, stacked in order. Sending cards without a
  `preset` turns `chat` into `split` (another preset stays); `[]`
  clears the cards and falls back to `chat`. A `screen` card, or an
  `alerts` card with a `crit` item, takes the whole content area.
- `status`: `top` and `bottom` each take `{"template", "items"}` or
  `null` (clear); a line left out stays.

Cards hold plain text and numbers, never HTML or URLs. Times are ISO
8601: `2026-11-14`, `2026-11-14T09:30`, optionally with seconds and
`Z` or `±HH:MM`; without an offset they are the tablet's local time.
Texts are non-empty; the limits below are in characters.

| `type` | Fields |
|--------|--------|
| `text` | `title?` (80), `body` (2000; blank lines split paragraphs) |
| `list` | `title?`, `items`: 1–12 × `{text (200), detail? (300), mark?: done \| open \| warn}` |
| `figures` | `title?`, `items`: 1–6 × `{label (40), value, unit? (12), delta?, tone?: good \| bad \| neutral}`; `value` and `delta` are numbers or text (24) |
| `alerts` | `title?`, `items`: 1–8 × `{level: crit \| warn \| info, title (120), detail?, since?: time}` |
| `calendar` | `title?`, `events`: 1–10 × `{start: time, end?: time, title (120), where? (80), all_day?: bool}` |
| `goals` | `title?`, `items`: 1–6 × `{name (80), progress: 0–1, current?, target?, due?: time}` |
| `chart` | `title?`, `kind: line \| bar`, `unit?`, `series`: 1–4 × `{name (40), points: [[x, y], ...]}` (500 points in all), `highlight?`: `{from, to}` or `{at}`. Every x is a number, or every x is text: times make a time axis, other text categories. |
| `transcript` | `lines?`: 1–50, how many recent lines the page shows |
| `screen` | `name` from the screens config, `params?`: `{name: value}`, each allowed by that screen's `params` |

A `screen` card reaches the page with the screen's `title` and its
`url`, built here: the configured URL plus the params as a query
string. The agent can't send either.

Status templates are built into the page; each has its own slots:

| Template | Slots |
|----------|-------|
| `desk` | `alerts`, `mail`: whole numbers 0–9999; `next`: text (60) |
| `note` | `text`: text (120) |

An unknown template or slot answers `422`. The page adds the
connection, the agent's state and the clock itself.

## What the user said

| Method | Path            | Auth  | Effect |
|--------|-----------------|-------|--------|
| POST   | `/heard`        | token | The page sends a finished utterance: `{"text": "..."}` (1–1000 characters). Answers `202 {"delivered_to": N}`, the number of adapters listening. When N > 0 the avatar turns `thinking`. |
| GET    | `/heard/events` | token | SSE stream for adapters: one `heard` event per utterance, `data: {"text": "..."}`. Nothing is buffered: an adapter sees only what is said while it is connected. |

## Events (server → page)

`GET /events` is a Server-Sent Events stream. On connect the server
sends the current display and avatar state (with `retry: 2000`, so a
dropped stream comes back in two seconds), then one event per change.
A `say` is never replayed, so a reconnect doesn't repeat a line.

```
event: display
data: {"preset":"focus","content":[{"type":"screen","name":"weather","params":{"range":"7d"},"title":"Weather","url":"https://weather.example.com/today?range=7d"}],"status":{"top":null,"bottom":null}}

event: avatar
data: {"state":"thinking"}

event: say
data: {"id":3,"text":"Done, three new mail.","audio":null}
```

`display` always carries the whole display. A `/display` call that only
sets `avatar` sends just an `avatar` event.

`audio` is the URL of rendered speech when the server has a speech
engine; `null` means the page speaks the text with the browser's
voice. With rendered speech, `say` will also carry
`words: [{"t_ms": 0, "text": "Done,"}, ...]`, each word's start in the
audio, and the page reveals the subtitle on that clock (the speech
engine is not built yet).
