# Changelog

Every deploy of agent-avatar is a release, versioned `vYYYY.MM.N` (N counts
from 0 each month). Each section is drafted from commit subjects by
git-cliff (`cliff.toml`) through `scripts/release.py`; big releases open
with a short summary. See `docs/RELEASING.md`.

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
