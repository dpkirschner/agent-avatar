# CLAUDE.md

agent-avatar: a generic display for an AI agent. A full-screen kiosk
page (sprite, screens, speech in/out) plus a small display server
(`show` / `sprite` / `say` over HTTP, SSE to the page) and adapters
(Matrix first). Vocabulary: `GLOSSARY.md`. Modeled on the owner's
outflow and vitals repos: same gate, release flow and leak scan.

- Gate: `make check` (fmt, clippy -D warnings, tests, `make leakscan`).
  Run it before committing.
- Layout: `server/` (Rust, axum), `app/` (plain HTML/JS, no build
  step, served as is), `examples/` (synthetic only), `scripts/`
  (release and leak scan), `docs/`.
- The repo is public and meant for reuse: no personal details (names,
  hostnames, tailnet names, real paths, real screens) in code, docs,
  fixtures or commit messages. Real screens configs stay outside the
  repo (`/screens.toml` is gitignored).
- Screens are names from the config, never URLs from the agent.
- Docs map: ARCHITECTURE (parts, data flow), API (endpoints, events),
  DEVELOPMENT (build, tests), RELEASING (versions, releases, leak
  scan). Update the matching doc with any behaviour change, and
  GLOSSARY.md when a term changes.
- Leak scan: `scripts/leakscan.py` is a copy of vitals' (outflow has
  the same minus the health rules); port fixes both ways. False
  shape-rule hits go in `.leakscan-allow`.

## Releases and branches (see docs/RELEASING.md)

Claude drives releases and repo mechanics, as in outflow and vitals.
Every deploy is a release, versioned `vYYYY.MM.N`; notes live in
`CHANGELOG.md`.

- While the first version takes shape, work merges to `main` as it
  goes. Once it is stable, switch to feature branches merged locally
  when whole (no PRs).
- Releases follow the `/release` skill: `make release-draft`, tidy,
  `make release`, show the user the notes and ask once (that yes
  covers `git push --atomic origin main V`), confirm the workflows
  went green, then deploy.
- Never move or delete a pushed tag; fix forward with the next N.
- Pre-push leak scan: a block means fix the commits; a warning means
  show the user and re-push with `LEAKSCAN_ACK=1` only after they say
  so.
