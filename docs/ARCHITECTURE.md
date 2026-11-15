# Architecture

Three parts, each replaceable: the display server, the kiosk page,
and adapters.

```
agent ──HTTP──▶ display server ──SSE──▶ kiosk page (tablet browser)
  ▲                                         │ speech-to-text
  └──────────────── adapter ◀───────────────┘
```

## Display server (`server/`)

A Rust binary (axum, tokio). It holds the avatar's current state (the
display: preset, cards and status lines; the avatar state; what it's
saying), takes commands from
agents over HTTP, and pushes each change to every connected page as an
SSE event. A page that connects gets the current state first, so a
reload or a reconnect after the tablet sleeps shows the right thing.

State lives in memory only (nothing on disk), so a restart starts idle
with no cards. The page (`app/`) is compiled into the binary with
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
| `AVATAR_THEME` | Which built-in theme the page wears (`app/themes/<name>/`), default `picture-book`. Unknown: the server refuses to start. |
| `AVATAR_FACE` | Which built-in avatar the page draws (`app/avatars/<name>/`), default `blob`. Unknown: the server refuses to start. |
| `RUST_LOG` | Log filter, default `info`. |

Source: `server/src/config.rs` (screens, token, look),
`server/src/assets.rs` (the embedded page, its `<head>`),
`server/src/display.rs` (card and status checks, presets),
`server/src/routes.rs` (state, routes, SSE), `server/src/main.rs` (startup).

## Kiosk page (`app/`)

Plain HTML, CSS and JS (no npm, no bundler). It draws the display,
listens to the SSE stream, plays speech, and runs the browser's speech
recognition for what the user says. The page is assembled from
folders, so components and looks come and go without touching the
rest:

| Folder | Holds |
|---|---|
| `app/core/` | `kit.js` (shared helpers and the card/status registry), `app.js` (layout, speech, SSE, settings), `layout.css` (structure, and every style token with neutral defaults). |
| `app/cards/<type>/` | `card.js` registers the card with `Kiosk.card(type, {render, claims?, mounted?, history?, settle?})`; `card.css` styles it with the tokens only. |
| `app/status/<name>/` | `status.js` registers a status template with `Kiosk.status(name, items => [...])`. |
| `app/themes/<name>/` | `theme.css` sets the tokens and adds the theme's own touches; its fonts (with licences) beside it. |
| `app/avatars/<name>/` | `avatar.js` and `avatar.css` (below). |

`server/build.rs` embeds the whole of `app/`; the server serves any
embedded `.js`, `.css`, font, image or text file by its path, and fills
`{{head}}` in `index.html` with every card and status module, the
configured theme (CSS last, so it wins) and the configured avatar.
Scripts are `defer` and run in order: kit, cards, status templates,
avatar, app.

- **Adding a card:** a folder `app/cards/<type>/` with `card.js` and
  `card.css`, and the server's check for it (a `Card` variant and its
  name in `CARD_TYPES`, `server/src/display.rs`). A test fails until
  the page folders and the server's list agree. Removing one is the
  same in reverse.
- **Adding a status template:** a folder `app/status/<name>/` and its
  slots in `TEMPLATES` (`display.rs`); the same test checks them.
- **Adding a theme:** a folder `app/themes/<name>/` with `theme.css`.
  Start from `plain` (the neutral defaults) and set tokens; card CSS
  never names a theme, so every card follows. `DESIGN.md` at the
  root records the default theme (`picture-book`): its palette, type,
  shapes and rules for new cards.

- **Layout.** A status line at the top (and one at the bottom when the
  agent sets it), the avatar column on the left, the content column on
  the right. The preset sets the proportions (`chat` 62/38, `split`
  half each, `focus` 24/76); with no cards the avatar has the whole
  stage. In portrait the columns become rows.
- **Status lines.** Each template's module draws its items (the server
  checks the slots); the page adds the agent's state, the
  connection dot and the clock. A dropped stream shows "offline" there,
  never a blank screen.
- **Cards.** Built from DOM nodes with `textContent` only, by each
  card's module. A card whose module `claims` it (a `screen`, an
  `alerts` card with a `crit` item) takes the content area alone.
  Charts are SVG drawn at the card's pixel size and redrawn on resize.
  A `screen` card is an iframe, kept across redraws while its URL
  stays.
- **Subtitles.** The agent's current line sits under the avatar,
  revealed word by word (browser voice: boundary events; server audio:
  `words` timings on the audio clock; neither: the whole line). The
  owner's last utterance shows small above it. A `transcript` card
  shows both sides' recent lines, kept in the page only.

- **Start tap.** The page opens behind a "Tap to start" overlay; the
  tap unlocks audio, takes a screen wake lock and asks for full screen.
- **Avatar module.** `app/avatars/<name>/avatar.js` sets
  `window.AvatarFace = {mount(el), state(s), mouth(v), size(preset)}`;
  its CSS stays under its own class. The page never looks inside it,
  so swapping the avatar touches nothing else; a theme dresses it only
  through the `--face-*` variables the avatar's CSS declares. What shows is `talking`
  while the page is speaking, else `listening` while the mic is on,
  else the server's avatar state. While talking the page drives
  `mouth` every frame: a word boundary opens it, and with no
  boundaries it moves on a gentle loop. To add one: a folder with both
  files; nothing else.
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
token) and calls `POST /say`. It is a separate process that uses only
the display server's HTTP API, so another chat system needs only a new
adapter. An agent can also call the display server directly to change
the display or the avatar state.

### Matrix adapter (`adapters/matrix/`)

A second binary, `agent-avatar-matrix`. It logs in to a homeserver as
its own user (the tablet's identity) and shares a room with the agent.

- **Speech in.** Each `heard` utterance is sent to the room as an
  `m.text` message.
- **Replies out.** It long-polls `/sync`, filtered to the room, and
  speaks the agent's `m.text` and `m.notice` messages. Agents that
  stream a reply post a message, then edit it (`m.replace`); the
  adapter keeps the newest `m.new_content` and speaks once the reply
  has gone `MATRIX_SETTLE_MS` without an edit. An edit that arrives
  after the reply was spoken and only adds to it (the agent paused
  mid-stream) is spoken as the new tail; any other late edit is
  dropped. Markdown, code blocks and reply quotes are stripped and
  URLs read as "link".
- **Avatar.** When the agent starts typing, the avatar turns
  `thinking` (`POST /display {"avatar": "thinking"}`); a `say` returns
  it to `idle`.
- **What it never speaks:** history from before it started (the first
  sync only finds the starting point), messages older than five
  minutes when they arrive (after an outage), anyone but the agent.
- **Failures.** A dropped connection on either side is retried with
  backoff. If the homeserver can't take an utterance, the tablet says
  "I couldn't reach the agent." A rejected token (401/403), a room it
  isn't in, or an end-to-end encrypted room stops it with a log line:
  it speaks the plain client-server API and can't read encrypted
  rooms.

Configuration (environment, read once at startup):

| Variable | Meaning |
|---|---|
| `AVATAR_URL` | The display server, default `http://127.0.0.1:8075`. |
| `AVATAR_TOKEN_FILE` | The display server's command token file, if it has one. |
| `MATRIX_HOMESERVER` | Homeserver base URL (client-server API). |
| `MATRIX_USER` | The adapter's user, `@name:server`. |
| `MATRIX_TOKEN_FILE` | 0600 file holding that user's access token (required; no plain-env form). |
| `MATRIX_ROOM` | Room id, `!id:server` (not an alias). |
| `MATRIX_AGENT_USER` | The agent's user: only its messages are spoken. |
| `MATRIX_SETTLE_MS` | Quiet time before a reply is spoken, default `1500`. |
| `RUST_LOG` | Log filter, default `info`. |

Source: `config.rs` (environment), `sync.rs` (the `/sync` response to
messages, edits and typing), `replies.rs` (settling edits), `speech.rs`
(text for speech), `sse.rs` (the heard stream), `http.rs` (homeserver
and display-server calls), `lib.rs` (the three loops).

## Screens config

A TOML file listing the screens an agent may show (name, title, URL,
and optional `[screen.params]`); see `examples/screens.toml`. Agents
pick screens by name only, so a compromised or confused agent can't
point the display at an arbitrary URL. A param is either a list of
allowed values or a kind: `"date"` (a real `YYYY-MM-DD`), `"month"`
(`YYYY-MM`), `"date_or_month"`, `"span"` (a date or month, or `a..b` of
those), `"word"` (1-40 letters, digits, spaces and `- _ & ' .`) or
`"int"`. The server checks each of the agent's values on its own (an
unknown param or a bad value is a `400`) and appends them as a query
string; rules between params are the framed page's to check, so an embeddable
chart view can take `metric`, `range` or `highlight` without the agent
ever writing a URL. The pages it lists must allow being framed by the avatar's origin
(`frame-ancestors`).
