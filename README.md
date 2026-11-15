# agent-avatar

A face and a voice for an AI agent, on a spare tablet.

agent-avatar turns a full-screen browser page (an iPad in Safari, say)
into a display your agent can drive: an animated sprite that shows
whether it's idle, listening, thinking or talking; screens it can swap
in (a status page, a dashboard, anything you list in a config file);
and speech in and out. You talk to the page; your words go to the
agent; the agent's replies come back spoken, and it can change what's
on screen while it works.

**Status: early.** The repo has its build, test and release tooling;
the page and the display server are being built now. See
[docs/API.md](docs/API.md) for the planned interface.

## How it fits together

```
agent ──HTTP (show / sprite / say)──▶ display server ──SSE──▶ kiosk page
  ▲                                                             │
  └────────── adapter (e.g. Matrix) ◀── speech-to-text ─────────┘
```

- **Kiosk page** (`app/`): plain HTML and JS, no build step.
- **Display server** (`server/`): a small Rust (axum) service. Agents
  call it over HTTP; it pushes events to the page over Server-Sent
  Events.
- **Adapters** connect the page's speech to an agent's chat (Matrix
  first).
- **Screens** come from a config file you keep outside the repo; see
  [examples/screens.toml](examples/screens.toml).

## Quick start

```
make serve      # display server on 127.0.0.1:8070
make check      # format, lint, tests, leak scan
```

Docs: [ARCHITECTURE](docs/ARCHITECTURE.md) ·
[API](docs/API.md) · [DEVELOPMENT](docs/DEVELOPMENT.md) ·
[RELEASING](docs/RELEASING.md) · [GLOSSARY](GLOSSARY.md)

## License

MIT. See [LICENSE](LICENSE).
