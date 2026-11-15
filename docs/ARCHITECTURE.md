# Architecture

Three parts, each replaceable.

```
agent ──HTTP──▶ display server ──SSE──▶ kiosk page (tablet browser)
  ▲                                         │ speech-to-text
  └──────────────── adapter ◀───────────────┘
```

## Display server (`server/`)

A Rust binary (axum, tokio). It holds the avatar's current state (the
screen, the sprite state, what it's saying), takes commands from
agents over HTTP, and pushes each change to every connected page as an
SSE event. A page that connects gets the current state first, so a
reload or a reconnect after the tablet sleeps shows the right thing.

State lives in memory only (nothing on disk), so a restart starts idle
with no screen. The page (`app/`) is compiled into the binary with
`include_str!`, so a release is one file.

It binds loopback by default (`AVATAR_ADDR`, default `127.0.0.1:8075`).
Put a reverse proxy in front for HTTPS and the network boundary; a
private network such as a tailnet is the intended setup. Speech
recognition in Safari needs HTTPS, so the page must come through that
proxy.

Configuration (environment, read once at startup):

| Variable | Meaning |
|---|---|
| `AVATAR_ADDR` | Listen address, default `127.0.0.1:8075`. |
| `AVATAR_SCREENS` | Path of the screens config. Unset: no screens. Unreadable or invalid: the server refuses to start. |
| `AVATAR_TOKEN_FILE` | 0600 file holding the command token (preferred). |
| `AVATAR_TOKEN` | The token itself, if a file isn't practical. |
| `RUST_LOG` | Log filter, default `info`. |

Source: `server/src/config.rs` (screens, token), `server/src/routes.rs`
(state, routes, SSE), `server/src/main.rs` (startup).

## Kiosk page (`app/`)

Plain HTML, CSS and JS (no npm, no bundler; `app/index.html`,
`app.js`, `app.css`). It renders the sprite and the current screen (an
iframe), listens to the SSE stream, plays speech, and runs the
browser's speech recognition for what the user says.

- **Start tap.** The page opens behind a "Tap to start" overlay; the
  tap unlocks audio, takes a screen wake lock and asks for full screen.
- **Sprite.** An inline SVG character animated with CSS; the
  `data-state` attribute on `#app` picks the look. What shows is
  `talking` while the page is speaking, else `listening` while the mic
  is on, else the server's sprite state.
- **Screens.** A `show` event with a URL fills the main area with an
  iframe and shrinks the sprite to a corner; `null` brings it back.
- **Speech out.** `say` lines queue and play in order: `audio` through
  one reused `<audio>` element, else `speechSynthesis`. The mic pauses
  while the avatar talks so it doesn't hear itself. Lines that arrive
  before the start tap are shown, not spoken.
- **Speech in.** Tap-to-talk by default; "Listen continuously" in
  settings keeps restarting recognition. Each finished utterance goes
  to `POST /heard`; if no adapter is listening, the page says so.
- **Settings.** Hold the top-left corner for 0.8 s: continuous
  listening, browser voice and rate (kept in `localStorage`), full
  screen, reload.

Browser notes from the prototype:

- On iOS, speech recognition works only in Safari; other iOS browsers
  are refused by the system.
- Safari often never marks a result final; use the last interim result
  when recognition ends. iOS ends each session after one utterance, so
  continuous listening means restarting on `end`.
- Audio the agent starts (not a tap) plays after one unlocking tap per
  page load. Reuse one `HTMLAudioElement`; Web Audio can hang after a
  lock.
- SSE drops while the tablet is locked and reconnects after unlock;
  send `retry:` so it comes back quickly.
- The browser's built-in voices are weak, so speech can come from a
  local TTS engine as audio the page plays.

## Adapters

An adapter carries what the user said on the page to an agent and the
agent's replies back as `say`. It reads `GET /heard/events` (with the
token) and calls `POST /say`. The first one talks to Matrix: speech
goes into the agent's chat, replies become speech. An agent can also
call the display server directly to change screens or the sprite.

## Screens config

A TOML file listing the screens an agent may show (name, title, URL);
see `examples/screens.toml`. Agents pick screens by name only, so a
compromised or confused agent can't point the display at an arbitrary
URL. The pages it lists must allow being framed by the avatar's origin
(`frame-ancestors`).
