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

It binds loopback by default (`AVATAR_ADDR`, default `127.0.0.1:8070`).
Put a reverse proxy in front for HTTPS and the network boundary; a
private network such as a tailnet is the intended setup.

## Kiosk page (`app/`)

Plain HTML, CSS and JS, served as is (no npm, no bundler). It renders
the sprite and the current screen (an iframe), listens to the SSE
stream, plays speech, and runs the browser's speech recognition for
what the user says.

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
agent's replies back as `say`. The first one talks to Matrix: speech
goes into the agent's chat, replies become speech. An agent can also
call the display server directly to change screens or the sprite.

## Screens config

A TOML file listing the screens an agent may show (name, title, URL);
see `examples/screens.toml`. Agents pick screens by name only, so a
compromised or confused agent can't point the display at an arbitrary
URL. The pages it lists must allow being framed by the avatar's origin
(`frame-ancestors`).
