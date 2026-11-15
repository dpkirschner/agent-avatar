# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

One owner and their AI agent. The owner keeps a spare tablet (an iPad
in Safari) propped on the desk in landscape, an arm's length to a
couple of metres away, and glances at it while working. They talk to
it; the agent answers out loud and changes what the tablet shows.
Portrait is a fallback, not a design target. The project is public so
other people can run it for their own agent; they are the same kind
of user, with their own screens.

## Product Purpose

agent-avatar gives an AI agent a face, a voice and a screen it
controls. The kiosk page shows the agent's presence (idle, listening,
thinking, talking) and whatever the agent decides to put next to it: a
configured screen such as a dashboard or status page, or content it
composes itself. Success: the owner can tell at a glance what the
agent is doing and read what it chose to show without walking over.

## Positioning

The agent composes the display. It decides how much room its own face
gets relative to the content, depending on what it is showing or what
the owner asked: the face alone while chatting, a small face beside a
dashboard, side by side while explaining something. A chat window or
a static dashboard can't do that.

## Operating Context

- Always-on kiosk: one start tap unlocks audio, keeps the screen
  awake and asks for full screen; after that it runs unattended for
  days and must recover from sleep, reloads and dropped connections
  by itself.
- Speech in through the browser's recognition, speech out through
  server audio (local TTS planned) or the browser voice.
- Driven over HTTP by the agent through a small display server, with
  SSE to the page. Adapters (Matrix first) carry speech to the agent's
  chat and replies back.

## Capabilities and Constraints

- Plain HTML/CSS/JS in `app/`, no build step, compiled into the Rust
  server binary. The page CSP forbids inline scripts and styles.
- Screens are names from a private config, never URLs from the agent.
- Layout model (decided 2026-11-14): a small set of named layout
  presets. The agent picks a preset and fills its slots; it never
  sends geometry.
- Agent-authored content (decided 2026-11-14): structured cards
  (typed content such as text, lists, key figures, tables, images
  from allowed sources) rendered in the page's own style. No raw HTML
  from the agent.
- The agent-facing API changes from `show`/`sprite` to layouts and
  frames; the agent's tool integration must follow it.
- Open: what the face is (the current SVG sprite, a richer character,
  or an abstract presence). Mouth or motion may later follow the
  speech audio once local TTS lands.

## Evidence on Hand

- The current page: an SVG sprite with four states and an iframe
  screen that shrinks the sprite to a corner (`app/`).
- `examples/screens.toml`: synthetic screens only. Real screens stay
  outside the repo; never commit screenshots of them.

## Product Principles

1. Readable at a glance from across a desk: state first, content
   second, chrome last.
2. The agent decides the layout; the page makes every choice look
   designed.
3. Unattended by default: every state, including errors and
   disconnects, must look intentional on a screen nobody is touching.
4. Safe to drive: the agent names things (screens, presets, card
   types); it never injects URLs or markup.
5. Generic and reusable: nothing about one owner's agent, screens or
   data is baked in.
