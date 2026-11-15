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
| GET    | `/`        | The kiosk page (`/app.js`, `/app.css` alongside). Its CSP frames only the configured screens' origins. |
| GET    | `/healthz` | `200 ok` |
| GET    | `/version` | `{"version": "v2026.11.0"}` (the release tag, or the crate version in a dev build) |
| GET    | `/state`   | `{"screen": {...}, "sprite": "idle", "last_say": {...} \| null}` |
| GET    | `/screens` | `[{"name": "status", "title": "Status"}]`: names and titles only, never URLs |

## Commands (agent → server, token)

| Method | Path      | Body                                | Effect |
|--------|-----------|-------------------------------------|--------|
| POST   | `/show`   | `{"screen": "status"}`              | Show a screen from the screens config. Unknown name → `404`. `{"screen": null}` clears it (the avatar alone). Returns the shown screen. |
| POST   | `/sprite` | `{"state": "thinking"}`             | Set the sprite state: `idle`, `listening`, `thinking`, `talking`. Anything else → `422`. |
| POST   | `/say`    | `{"text": "Done, three new mail."}` | Speak a line (trimmed, 1–2000 characters, else `400`). Returns `{"id", "text", "audio"}`. The page shows `talking` while it plays; the sprite state becomes `idle`. |

## What the user said

| Method | Path            | Auth  | Effect |
|--------|-----------------|-------|--------|
| POST   | `/heard`        | token | The page sends a finished utterance: `{"text": "..."}` (1–1000 characters). Answers `202 {"delivered_to": N}`, the number of adapters listening. When N > 0 the sprite turns `thinking`. |
| GET    | `/heard/events` | token | SSE stream for adapters: one `heard` event per utterance, `data: {"text": "..."}`. Nothing is buffered: an adapter sees only what is said while it is connected. |

## Events (server → page)

`GET /events` is a Server-Sent Events stream. On connect the server
sends the current screen and sprite state (with `retry: 2000`, so a
dropped stream comes back in two seconds), then one event per change.
A `say` is never replayed, so a reconnect doesn't repeat a line.

```
event: show
data: {"screen":"status","title":"Status","url":"https://status.example.com/"}

event: show
data: {"screen":null,"title":null,"url":null}

event: sprite
data: {"state":"thinking"}

event: say
data: {"id":3,"text":"Done, three new mail.","audio":null}
```

`audio` is the URL of rendered speech when the server has a speech
engine; `null` means the page speaks the text with the browser's
voice.
