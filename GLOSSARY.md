# Glossary

Words used in agent-avatar's code, docs and API, with the meaning they
have here.

- **Avatar**: the whole display an agent drives: the sprite, the
  current screen, and the voice.
- **Kiosk page**: the full-screen web page that renders the avatar on
  the tablet. One page, served by the display server.
- **Display server**: the HTTP service that agents call and that pushes
  events to the kiosk page over SSE.
- **Screen**: a named page the avatar can show in its main area, listed
  in the screens config. The agent picks one with `show <name>`; it
  can't show arbitrary URLs.
- **Screens config**: the file that lists screens (name, title, URL).
  Yours stays private; `examples/screens.toml` is the synthetic sample.
- **Sprite**: the animated character. It is always in exactly one
  sprite state.
- **Sprite state**: one of `idle`, `listening`, `thinking`, `talking`.
- **Say**: an agent asks the avatar to speak a line of text. The page
  plays the audio and the sprite is `talking` while it does.
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
  (`show`, `sprite`, `say`), or to an adapter (`heard`).
