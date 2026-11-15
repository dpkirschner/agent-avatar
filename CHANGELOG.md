# Changelog

Every deploy of agent-avatar is a release, versioned `vYYYY.MM.N` (N counts
from 0 each month). Each section is drafted from commit subjects by
git-cliff (`cliff.toml`) through `scripts/release.py`; big releases open
with a short summary. See `docs/RELEASING.md`.

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
