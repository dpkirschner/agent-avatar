# Development

Prereqs: a stable Rust toolchain (`rust-toolchain.toml`), Python 3 for
the scripts, and `git-cliff` for drafting release notes
(`brew install git-cliff`).

```
make            # list targets
make serve      # display server on 127.0.0.1:8075 (AVATAR_ADDR overrides)
make test       # Rust tests
make check      # the gate: fmt, clippy -D warnings, tests, leak scan
```

The page in `app/` has no build step, but it is compiled into the
server binary: after editing it, restart `make serve`, then reload.
No inline `<script>` or `style=""`: the page CSP refuses them.

Drive a local server by hand:

```
B=http://127.0.0.1:8075; J='Content-Type: application/json'
curl -X POST -H "$J" -d '{"screen":"status"}' $B/show
curl -X POST -H "$J" -d '{"state":"thinking"}' $B/sprite
curl -X POST -H "$J" -d '{"text":"Hello."}' $B/say
curl -N $B/heard/events     # what the page hears
```

Add `-H "Authorization: Bearer $TOKEN"` when the server has a token.

Tests: Rust unit and route tests live next to the code
(`tower::ServiceExt::oneshot` for routes). The release, deploy and leak-scan
scripts have their own tests in `scripts/test_*.py`, run by
`make leakscan` (the deploy tests use throwaway plists and need macOS).

Data: everything under `examples/` is invented and says "synthetic"
near the top. Never copy a real screens config or a capture into the
repo.
