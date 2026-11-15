# Deployment (always-on Mac, launchd, a private-network proxy)

A LaunchDaemon runs the display server on loopback; an HTTPS proxy on a
private network (here `tailscale serve`, tailnet only) puts the page in
front of the tablet. Agents on the same machine, including containers,
call the loopback port. `scripts/deploy.sh` does the work; its header
comment is the full reference.

## 1. Set up the host (once)

In a terminal (sudo asks for a password):

```
scripts/deploy.sh setup             # --addr 127.0.0.1:PORT to change the port
```

This creates `~/Library/Application Support/agent-avatar/` (0700) with:

- `token`: a random command token (0600, never printed). Give it to
  the agent or adapter that drives the display, and paste it once into
  the page's settings on the kiosk (the page needs it for `POST /heard`).
- `screens.toml`: a copy of the synthetic example. Edit it to list your
  own pages; it stays on the host, never in the repo.
- `releases/`: one folder per installed release.

and `/Library/LaunchDaemons/com.agent-avatar.server.plist` (root:wheel
644), which runs `releases/current/agent-avatar-server` as you, with
`AVATAR_ADDR`, `AVATAR_SCREENS` and `AVATAR_TOKEN_FILE` set, logging to
`~/Library/Logs/agent-avatar.log`. Nothing starts until the first
install. Each release holds both binaries, the server and the Matrix
adapter; the adapter only runs once `setup-matrix` installs its plist
(section 5).

## 2. Release, build, install

Cut and publish the release first (`/release`, [RELEASING.md](RELEASING.md)), then:

```
scripts/deploy.sh build vYYYY.MM.N      # no sudo; compiles in ~/agent-avatar-build
scripts/deploy.sh install vYYYY.MM.N    # terminal (sudo): stop, switch, start, check
scripts/deploy.sh verify                # no sudo: what's live, checks again
```

The install checks: `/healthz`, the page, `/state`, `/status`, `/screens` (it
prints the screen names), that a command without the token gets `401`,
and that `/version` reports the tag. `scripts/deploy.sh rollback`
switches back to the previous release; `status` shows what's live and
installed.

Before releasing, `scripts/deploy.sh try <branch>` runs a branch on
127.0.0.1:8077 with the live screens config and a scratch token, so
nothing live changes.

## 3. Settings

`scripts/deploy.sh env [--matrix] list|set|unset|restore` edits the plist's
environment with a diff, a confirmation and a backup. Secret-looking
keys are refused (the plist is world-readable): use the `_FILE` form.
A change takes effect at the next start (`env set ... --restart`).

The screens config is read at startup: after editing it, restart with
`sudo launchctl kickstart -k system/com.agent-avatar.server`. An invalid
file stops the server from starting; check the log.

## 4. The proxy (HTTPS on the private network)

Safari's speech recognition needs HTTPS. With Tailscale:

```
tailscale serve --bg --https=PORT http://127.0.0.1:8075
```

and open `https://HOST.TAILNET.ts.net:PORT/` in Safari on the tablet.
The route is tailnet only (not Funnel). Remove it with
`tailscale serve --https=PORT off`.

## 5. Agents and adapters

From the same machine: `http://127.0.0.1:8075`. From a container on it
(colima, Docker Desktop), the host's loopback is usually
`http://host.docker.internal:8075`. Send the token as
`Authorization: Bearer <token>`; read it from the token file into the
agent's own secret store rather than copying it into a config that
gets committed.

### Matrix adapter (optional)

Give the adapter its own Matrix user, joined to an unencrypted room
with the agent. Put that user's access token in the data dir, 0600,
without it touching argv or shell history (paste, then Ctrl-D):

```
D="$HOME/Library/Application Support/agent-avatar"
(umask 077 && cat >"$D/matrix-token")
```

Then, in a terminal (sudo):

```
scripts/deploy.sh setup-matrix --homeserver http://127.0.0.1:8008 --user @tablet:example.org --room '!id:example.org' --agent @agent:example.org
```

This writes `/Library/LaunchDaemons/com.agent-avatar.matrix.plist`,
which runs `releases/current/agent-avatar-matrix` with the display
server's address and token file, logging to
`~/Library/Logs/agent-avatar-matrix.log`. If the live release has the
adapter, it starts now ("adapter up" in the log); otherwise with the
next install. From then on `install`, `rollback` and
`env --restart` stop and start it with the server, and
`scripts/deploy.sh env --matrix ...` edits its settings. Quote the room
id: `!` means something to the shell.

## 6. Screens that allow framing

Each page in the screens config must allow being framed by the
avatar's origin: no `X-Frame-Options: DENY`/`SAMEORIGIN`, and if it
sends a CSP, `frame-ancestors` must include
`https://HOST.TAILNET.ts.net:PORT`. The kiosk page's own CSP only allows
framing the origins listed in the screens config.

## 7. Tablet

In Safari: open the URL, tap "Tap to start", paste the token from the
token file into the settings that open, then allow the microphone on
the first "Talk". For a kiosk, use Add to Home Screen, or Guided
Access (Settings › Accessibility) to lock the tablet to Safari. Other
iOS browsers can't use speech recognition.
