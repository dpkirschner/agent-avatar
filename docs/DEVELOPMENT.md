# Development

Prereqs: a stable Rust toolchain (`rust-toolchain.toml`), Python 3 for
the scripts, and `git-cliff` for drafting release notes
(`brew install git-cliff`).

```
make            # list targets
make serve      # display server on 127.0.0.1:8070 (AVATAR_ADDR overrides)
make test       # Rust tests
make check      # the gate: fmt, clippy -D warnings, tests, leak scan
```

The page in `app/` has no build step: edit and reload.

Tests: Rust unit and route tests live next to the code
(`tower::ServiceExt::oneshot` for routes). The release and leak-scan
scripts have their own tests in `scripts/test_*.py`, run by
`make leakscan`.

Data: everything under `examples/` is invented and says "synthetic"
near the top. Never copy a real screens config or a capture into the
repo.
