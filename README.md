# agent-avatar

A face and a voice for an AI agent, on a spare tablet.

agent-avatar turns a full-screen browser page (an iPad in Safari, say)
into a display your agent can drive: an animated face that shows
whether it's idle, listening, thinking or talking, with subtitles as it
speaks; cards it fills with what it's talking about (figures, charts,
a calendar, alerts, lists) or pages you list in a config file; and
speech in and out. The agent picks how the face and the content share
the screen. You talk to the page; your words go to the
agent; the agent's replies come back spoken, and it can change what's
on screen while it works.

**Status: early.** The page, the display server and the Matrix adapter
work; local speech is next. See [docs/API.md](docs/API.md).

## How it fits together

```
agent ──HTTP (display / say)──▶ display server ──SSE──▶ kiosk page
  ▲                                                             │
  └────────── adapter (e.g. Matrix) ◀── speech-to-text ─────────┘
```

- **Kiosk page** (`app/`): plain HTML and JS, no build step.
- **Display server** (`server/`): a small Rust (axum) service. Agents
  call it over HTTP; it pushes events to the page over Server-Sent
  Events.
- **Adapters** (`adapters/`) connect the page's speech to an agent's
  chat. The Matrix adapter posts what you say to a room and speaks the
  agent's replies.
- **Screens** come from a config file you keep outside the repo; see
  [examples/screens.toml](examples/screens.toml).

## Quick start

```
cp examples/screens.toml screens.toml   # then point it at your pages
AVATAR_SCREENS=screens.toml make serve  # http://127.0.0.1:8075/
curl -X POST -H 'Content-Type: application/json' -d '{"text":"Hello."}' http://127.0.0.1:8075/say
make check      # format, lint, tests, leak scan
```

Speech recognition in Safari needs HTTPS, so on a tablet put the
server behind an HTTPS proxy on a private network; see
[docs/DEPLOYMENT.md](docs/DEPLOYMENT.md).

Docs: [ARCHITECTURE](docs/ARCHITECTURE.md) ·
[API](docs/API.md) · [DEVELOPMENT](docs/DEVELOPMENT.md) ·
[DEPLOYMENT](docs/DEPLOYMENT.md) ·
[RELEASING](docs/RELEASING.md) · [GLOSSARY](GLOSSARY.md)

## License

MIT. See [LICENSE](LICENSE).
