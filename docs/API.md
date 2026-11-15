# API

**Planned.** Only `GET /healthz` exists today. This is the interface
the display server is being built to; it may change before the first
release.

## Health

| Method | Path       | Response   |
|--------|------------|------------|
| GET    | `/healthz` | `200 ok`   |

## Commands (agent → server)

JSON bodies, snake_case fields.

| Method | Path      | Body                              | Effect |
|--------|-----------|-----------------------------------|--------|
| POST   | `/show`   | `{"screen": "status"}`            | Show a screen from the screens config. Unknown name → 404. |
| POST   | `/sprite` | `{"state": "thinking"}`           | Set the sprite state: `idle`, `listening`, `thinking`, `talking`. |
| POST   | `/say`    | `{"text": "Done, three new mail."}` | Speak a line. The sprite is `talking` while it plays. |
| GET    | `/state`  |                                   | The current screen, sprite state and line being spoken. |
| GET    | `/screens`|                                   | The screens an agent may show (names and titles). |

## Events (server → page)

`GET /events` is a Server-Sent Events stream. On connect the server
sends the current state, then one event per change:

```
event: show
data: {"screen":"status","title":"Status","url":"https://status.example.com/"}

event: sprite
data: {"state":"thinking"}

event: say
data: {"text":"Done, three new mail.","audio":"/audio/123.wav"}
```

## Auth

Planned: an optional bearer token for commands, so only the agent can
drive the display; the page and the event stream sit behind the same
private network.
