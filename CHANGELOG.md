# Changelog

Every deploy of agent-avatar is a release, versioned `vYYYY.MM.N` (N counts
from 0 each month). Each section is drafted from commit subjects by
git-cliff (`cliff.toml`) through `scripts/release.py`; big releases open
with a short summary. See `docs/RELEASING.md`.

## v2026.11.5 (2026-11-14)

### Fixes

- app: Speech recognition errors show on the page instead of the Talk button silently resetting. A browser without a speech service (Brave and other Chromium builds) is told to use Safari or Chrome (ho-ru7.6) ([9a9ee9e](https://github.com/dpkirschner/agent-avatar/commit/9a9ee9e))
- server: `/status` `page_seen_secs` counts from server start until a page connects, never `null`, so a monitor can't read "no page yet" as fresh (ho-ru7.9) ([4aaebd8](https://github.com/dpkirschner/agent-avatar/commit/4aaebd8))

## v2026.11.4 (2026-11-14)

### Features

- server: Open `GET /status` for monitoring: `{"pages", "page_seen_secs", "adapters"}`, the pages and adapters connected now and seconds since a page was last connected. Counts only, no content. The install checks fetch it (ho-ru7.9) ([617c94f](https://github.com/dpkirschner/agent-avatar/commit/617c94f), [15d939d](https://github.com/dpkirschner/agent-avatar/commit/15d939d))

### Fixes

- deploy: `status` reads the adapter's launchd state without sudo, so it no longer says "not loaded" for a running adapter (ho-ru7.7) ([71a66e3](https://github.com/dpkirschner/agent-avatar/commit/71a66e3))

## v2026.11.3 (2026-11-14)

The Matrix adapter: what you say to the tablet goes to the agent's
Matrix room, and the agent's replies come back spoken.

### Features

- matrix: New `agent-avatar-matrix` binary. Posts each utterance to a room as its own user and speaks the agent's text and notice replies, once each has gone 1.5 s without an edit (`MATRIX_SETTLE_MS`), so a streamed reply is said once. The agent typing turns the sprite `thinking`. It never speaks history, replies older than five minutes, or anyone but the agent; it refuses an end-to-end encrypted room and stops on a rejected token (ho-ru7.7) ([939a2ea](https://github.com/dpkirschner/agent-avatar/commit/939a2ea))
- deploy: Releases carry both binaries. `deploy.sh setup-matrix` installs the adapter's LaunchDaemon (its access token in a 0600 file in the data dir); install, rollback and `env --restart` stop and start it with the server, and `env --matrix` edits its settings (ho-ru7.7) ([eb25112](https://github.com/dpkirschner/agent-avatar/commit/eb25112))

## v2026.11.2 (2026-11-14)

### Fixes

- server: `POST /heard` needs the command token, like the commands. Its text reaches the agent as if the owner said it, so a tailnet device without the token could otherwise speak for the owner. The page asks for the token on the first start tap (a Token field in settings), keeps it, and reopens settings if the server refuses it (ho-ru7.6) ([33ec945](https://github.com/dpkirschner/agent-avatar/commit/33ec945), [ced3f71](https://github.com/dpkirschner/agent-avatar/commit/ced3f71), [24ba3a1](https://github.com/dpkirschner/agent-avatar/commit/24ba3a1))

## v2026.11.1 (2026-11-14)

The first deployed release; v2026.11.0 was tagged but never installed.

### Fixes

- server: Check the command token before reading the body, so a tokenless command gets 401 rather than 415 (ho-ru7.6) ([c9d6fc9](https://github.com/dpkirschner/agent-avatar/commit/c9d6fc9))

## v2026.11.0 (2026-11-14)

First tagged release: a full-screen avatar page for a tablet kiosk and the
small server that drives it. An agent can show a named screen, set the
sprite's state and make it speak; the page listens and posts back what it
heard.

### Features

- Display server: bearer-token HTTP API (`show`, `sprite`, `say`), an SSE
  stream to the page, `POST /heard` with a token-guarded
  `/heard/events` stream for the agent, and screens loaded by name from a
  TOML file kept outside the repo.
- Kiosk page compiled into the binary, with a CSP that frames only the
  configured screen origins: start tap, SVG sprite (idle, listening,
  thinking, talking), iframe screens with a corner sprite, tap-to-talk or
  continuous listening, queued speech, and settings behind a corner hold.
- `scripts/deploy.sh` (try, build, install, rollback, verify, status) plus
  a `setup` step that writes the launchd plist, a 0600 token file and a
  starter `screens.toml`; see `docs/DEPLOYMENT.md`.
