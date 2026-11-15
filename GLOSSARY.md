# Glossary

Words used in agent-avatar's code, docs and API, with the meaning they
have here.

- **Avatar**: the agent's face on the page, drawn by an avatar module
  (`app/avatars/<name>/`), with a state (`idle`, `listening`,
  `thinking`, `talking`) and a moving mouth. Chosen with `AVATAR_FACE`.
- **Display**: everything the agent sets with `POST /display`: the
  preset, the cards and the status lines (plus the avatar state).
- **Preset**: how the avatar and the content share the stage: `chat`,
  `split` or `focus`. The agent picks one; the page owns the geometry.
- **Card**: one typed block of content (text, list, figures, alerts,
  calendar, goals, chart, transcript, screen). At most three at once.
- **Status line**: the thin band at the top or bottom, drawn from a
  built-in template with the agent's values in its slots.
- **Subtitles**: the agent's current line under the avatar, revealed
  word by word as it speaks.
- **Kiosk page**: the full-screen web page that renders the avatar on
  the tablet. One page, served by the display server.
- **Display server**: the HTTP service that agents call and that pushes
  events to the kiosk page over SSE.
- **Screen**: a named page the agent can show in a `screen` card,
  listed in the screens config, with any params the config allows. The
  agent can't show arbitrary URLs.
- **Screens config**: the file that lists screens (name, title, URL,
  allowed params).
  Yours stays private; `examples/screens.toml` is the synthetic sample.
- **Say**: an agent asks the avatar to speak a line of text. The page
  plays the audio and the avatar is `talking` while it does.
- **Heard**: a finished utterance the page recognised from the user's
  speech, sent to `POST /heard` and passed to adapters.
- **Command token**: the bearer token agents and adapters present to
  drive the display. The page itself never holds it.
- **Adapter**: the piece that carries what the user says on the page
  to an agent's chat, and the agent's replies back to the avatar
  (Matrix first). A separate process using only the display server's
  HTTP API.
- **Settle**: a reply is settled once it has gone `MATRIX_SETTLE_MS`
  without an edit; only then is it spoken, so a streamed reply is said
  once.
- **Event**: one message from the display server to the page over SSE
  (`display`, `say`), or to an adapter (`heard`).
