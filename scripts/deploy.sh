#!/bin/bash
# deploy.sh: build and install agent-avatar releases on a macOS launchd host.
#
#   scripts/deploy.sh setup [--addr HOST:PORT]
#   scripts/deploy.sh setup-matrix --homeserver URL --user @U:S --room !R:S --agent @A:S
#   scripts/deploy.sh try <ref> [--port P] [--check SCRIPT]
#   scripts/deploy.sh try --clean
#   scripts/deploy.sh build <tag> [--rebuild]
#   scripts/deploy.sh install <tag> [--check SCRIPT]
#   scripts/deploy.sh rollback [<tag>]
#   scripts/deploy.sh env [--matrix] list
#   scripts/deploy.sh env [--matrix] set|unset|restore [--yes] [--restart] ARGS
#   scripts/deploy.sh verify [--log RE]... [--check SCRIPT]
#   scripts/deploy.sh status
#
# A trimmed copy of vitals' deploy.sh: the same releases layout, env
# editing and checks, minus everything about a database (the display
# server keeps no data). Every deploy is a release (docs/RELEASING.md).
# Each release lives in <data dir>/releases/<tag>/ (agent-avatar-server,
# agent-avatar-matrix, BUILD; the page is compiled into the server) and
# launchd runs releases/current, a symlink. Installing or rolling back switches that
# link, so the development checkout is never what's live.
#
# setup    Once per host, in a terminal (sudo). Creates the data dir
#          (0700), a random command token in <data dir>/token (0600,
#          never printed), a screens config copied from examples/ if
#          there is none, and the LaunchDaemon plist (root:wheel 644)
#          pointing at releases/current. Starts nothing: the first
#          install does.
# setup-matrix  Once per host, after setup, in a terminal (sudo). The
#          Matrix adapter's LaunchDaemon (com.agent-avatar.matrix): it
#          talks to this host's display server with its token, and to the
#          homeserver as --user, in --room, speaking --agent's replies.
#          Put the user's access token in <data dir>/matrix-token (0600)
#          first; setup-matrix refuses without it. Starts the adapter if
#          the live release has it; from then on install, rollback and
#          env --restart stop and start it with the server.
# try      No sudo; nothing live changes. Builds <ref> (a branch or
#          commit), starts it on 127.0.0.1:8077 (--port) with the live
#          screens config and a scratch token, runs the install checks,
#          then waits. Ctrl-C or TERM stops it; --clean does the same
#          after a kill -9.
# build    No sudo. Checks the tag is a published release on origin/main,
#          checks it out in a separate build worktree, compiles with
#          AVATAR_VERSION=<tag> (so /version reports it), and stages
#          releases/<tag>/. Nothing live changes.
# install  Needs sudo, so run it in a terminal. Stops the server (if
#          running), points current at <tag>, starts, then checks the
#          endpoints and /version. --check runs `python3 SCRIPT DIR` on
#          the fetched JSON. Prunes to the newest $KEEP_RELEASES.
# rollback Points current back at <tag>, default the release before the
#          live one.
# env      The plist's EnvironmentVariables (--matrix: the adapter's
#          plist). list prints them. set
#          KEY=VALUE..., unset KEY... and restore [FILE] (default: the
#          newest backup) edit a temp copy, lint it, show the diff, ask,
#          back up the live plist to <data dir>/plist-backups/, and
#          install it root:wheel 644 (sudo). Secret-looking keys (*TOKEN,
#          *SECRET, *PASSWORD, *KEY) are refused: the plist is world
#          readable, so it names a 0600 file (*_FILE) instead. A change
#          takes effect at the next start: install, rollback, or
#          --restart now. --yes skips the question (tests).
# verify   No sudo. Reports only what it observed: the live tag against
#          /version, the install checks (plus --check), and each --log
#          pattern in the log lines written since the live release
#          started. Exits 1 if anything is missing.
# status   What's live, what's installed, and the install history.
#
# Settings come from the launchd plist (data dir, address, token file,
# log); override with env: AVATAR_PLIST, AVATAR_MATRIX_PLIST,
# AVATAR_BUILD_DIR (default ~/agent-avatar-build), AVATAR_DATA_DIR (setup only), KEEP_RELEASES (5),
# KEEP_PLIST_BACKUPS (10). Tests set LAUNCHCTL and SUDO.
set -euo pipefail

PLIST=${AVATAR_PLIST:-/Library/LaunchDaemons/com.agent-avatar.server.plist}
MATRIX_PLIST=${AVATAR_MATRIX_PLIST:-/Library/LaunchDaemons/com.agent-avatar.matrix.plist}
BUILD_DIR=${AVATAR_BUILD_DIR:-$HOME/agent-avatar-build}
KEEP_RELEASES=${KEEP_RELEASES:-5}
KEEP_PLIST_BACKUPS=${KEEP_PLIST_BACKUPS:-10}
LAUNCHCTL=${LAUNCHCTL:-launchctl}
SUDO=${SUDO-sudo}
REPO=$(cd "$(dirname "$0")/.." && pwd)
BIN=agent-avatar-server
MATRIX_BIN=agent-avatar-matrix
UP_LINE="server up"
MATRIX_UP="adapter up"
DEFAULT_ADDR=127.0.0.1:8075
TAG_RE='^v[0-9]{4}\.[0-9]{2}\.[0-9]+$'
TRY_SRC="$BUILD_DIR/try-src"
TRY_BIN="$BUILD_DIR/try-bin"
TRY_DATA="$BUILD_DIR/try-data"
TRY_PIDFILE="$BUILD_DIR/try.pid"

STOPPED=0 SWITCHED=0
die() {
  echo "deploy: $*" >&2
  if [ "$STOPPED" = 1 ] || [ "$SWITCHED" = 1 ]; then
    [ "$STOPPED" = 0 ] || echo "deploy: the server may be down." >&2
    echo "deploy: restore the previous release with: scripts/deploy.sh rollback" >&2
  fi
  exit 1
}
step() { printf '\n--- %s\n' "$*"; }
pb() { /usr/libexec/PlistBuddy -c "Print :$1" "${2:-$PLIST}" 2>/dev/null || true; }

load_plist() {
  [ -r "$PLIST" ] || die "can't read $PLIST (first time on this host? scripts/deploy.sh setup)"
  LABEL=$(pb Label)
  PROG=$(pb ProgramArguments:0)
  DATA=$(pb EnvironmentVariables:AVATAR_DATA_DIR)
  DATA=${DATA:-$HOME/Library/Application Support/agent-avatar}
  LISTEN=$(pb EnvironmentVariables:AVATAR_ADDR)
  LISTEN=${LISTEN:-$DEFAULT_ADDR}
  TOKEN_FILE=$(pb EnvironmentVariables:AVATAR_TOKEN_FILE)
  SCREENS=$(pb EnvironmentVariables:AVATAR_SCREENS)
  LOG=$(pb StandardOutPath)
  [ -n "$LABEL" ] && [ -n "$PROG" ] || die "$PLIST has no Label or ProgramArguments"
  REL="$DATA/releases"
  CUR="$REL/current"
}

check_tag() {
  [[ $1 =~ $TAG_RE ]] || die "$1 isn't a release tag (vYYYY.MM.N)"
}

live_tag() { # the tag current points at, or empty
  [ -L "$CUR" ] && basename "$(readlink "$CUR")" || true
}

previous_tag() { # newest history entry that isn't live and still exists
  local live t
  live=$(live_tag)
  [ -f "$REL/history" ] || return 0
  # history lines: date time tag [note]
  tail -r "$REL/history" | awk '{print $3}' | while read -r t; do
    if [ "$t" != "$live" ] && [ -d "$REL/$t" ]; then echo "$t"; break; fi
  done
}

install_plist() { # file [dest]: install it as the live plist, root:wheel 644
  local dest=${2:-$PLIST}
  if [ -n "$SUDO" ]; then
    $SUDO install -m 644 -o root -g wheel "$1" "$dest"
  else
    install -m 644 "$1" "$dest"
  fi
}

# ── setup ──────────────────────────────────────────────────────────────
cmd_setup() { # addr
  local addr=$1
  [ ! -e "$PLIST" ] || die "$PLIST exists; change it with: scripts/deploy.sh env"
  [ -t 0 ] || [ -z "$SUDO" ] || die "run setup in a terminal (sudo asks for a password)"
  [[ $addr =~ ^[0-9.]+:[0-9]+$ ]] || die "--addr takes HOST:PORT, got $addr"
  local data=${AVATAR_DATA_DIR:-$HOME/Library/Application Support/agent-avatar}
  local logf="$HOME/Library/Logs/agent-avatar.log"
  step "data dir $data"
  (umask 077 && mkdir -p "$data/releases")
  chmod 700 "$data"
  if [ -e "$data/token" ]; then
    echo "keeping the existing token file"
  else
    (umask 077 && openssl rand -hex 32 >"$data/token")
    echo "wrote a new command token to $data/token (0600; give it to the agent)"
  fi
  if [ -e "$data/screens.toml" ]; then
    echo "keeping the existing screens config"
  else
    cp "$REPO/examples/screens.toml" "$data/screens.toml"
    chmod 600 "$data/screens.toml"
    echo "copied the example screens config to $data/screens.toml: edit it"
  fi
  step "plist $PLIST"
  local new
  new=$(mktemp "${TMPDIR:-/tmp}/avatar-plist.XXXXXX")
  /usr/bin/python3 -I - "$new" "$data" "$addr" "$logf" "$(id -un)" <<'EOF'
import plistlib, sys
out, data, addr, logf, user = sys.argv[1:]
plistlib.dump({
    "Label": "com.agent-avatar.server",
    "UserName": user,
    "GroupName": "staff",
    "ProgramArguments": [f"{data}/releases/current/agent-avatar-server"],
    "WorkingDirectory": data,
    "EnvironmentVariables": {
        "AVATAR_DATA_DIR": data,
        "AVATAR_ADDR": addr,
        "AVATAR_SCREENS": f"{data}/screens.toml",
        "AVATAR_TOKEN_FILE": f"{data}/token",
    },
    "RunAtLoad": True,
    "KeepAlive": True,
    "StandardOutPath": logf,
    "StandardErrorPath": logf,
}, open(out, "wb"))
EOF
  plutil -lint -s "$new" || die "generated plist doesn't lint"
  install_plist "$new"
  rm -f "$new"
  echo "installed $PLIST (listens on $addr; nothing started yet)"
  echo "next: scripts/deploy.sh build <tag>, then scripts/deploy.sh install <tag>"
}

# ── setup-matrix ───────────────────────────────────────────────────────
cmd_setup_matrix() { # homeserver user room agent
  local hs=$1 user=$2 room=$3 agent=$4 id_re='^[@!][^[:space:]:]+:[^[:space:]]+$'
  [ -n "$hs" ] && [ -n "$user" ] && [ -n "$room" ] && [ -n "$agent" ] ||
    die "usage: deploy.sh setup-matrix --homeserver URL --user @U:S --room !R:S --agent @A:S"
  [[ $hs =~ ^https?://[^[:space:]]+$ ]] || die "--homeserver takes an http(s) URL"
  [[ $user =~ $id_re && $user == @* ]] || die "--user takes @name:server"
  [[ $agent =~ $id_re && $agent == @* ]] || die "--agent takes @name:server"
  [[ $room =~ $id_re && $room == !* ]] || die "--room takes a room id, !id:server"
  load_plist
  [ ! -e "$MATRIX_PLIST" ] || die "$MATRIX_PLIST exists; change it with: scripts/deploy.sh env --matrix"
  [ -t 0 ] || [ -z "$SUDO" ] || die "run setup-matrix in a terminal (sudo asks for a password)"
  local tokf="$DATA/matrix-token"
  [ -f "$tokf" ] || die "put $user's access token in $tokf (0600) first"
  [ "$(stat -f %Lp "$tokf")" = 600 ] || die "$tokf must be mode 0600 (chmod 600 it)"
  local logf="$HOME/Library/Logs/agent-avatar-matrix.log"
  step "plist $MATRIX_PLIST"
  local new
  new=$(mktemp "${TMPDIR:-/tmp}/avatar-plist.XXXXXX")
  /usr/bin/python3 -I - "$new" "$DATA" "$LISTEN" "$TOKEN_FILE" "$tokf" "$hs" "$user" \
    "$room" "$agent" "$logf" "$(id -un)" <<'EOF'
import plistlib, sys
out, data, listen, avatar_tok, tokf, hs, user, room, agent, logf, who = sys.argv[1:]
env = {
    "AVATAR_DATA_DIR": data,
    "AVATAR_URL": f"http://{listen}",
    "MATRIX_HOMESERVER": hs,
    "MATRIX_USER": user,
    "MATRIX_ROOM": room,
    "MATRIX_AGENT_USER": agent,
    "MATRIX_TOKEN_FILE": tokf,
}
if avatar_tok:
    env["AVATAR_TOKEN_FILE"] = avatar_tok
plistlib.dump({
    "Label": "com.agent-avatar.matrix",
    "UserName": who,
    "GroupName": "staff",
    "ProgramArguments": [f"{data}/releases/current/agent-avatar-matrix"],
    "WorkingDirectory": data,
    "EnvironmentVariables": env,
    "RunAtLoad": True,
    "KeepAlive": True,
    "ThrottleInterval": 30,
    "StandardOutPath": logf,
    "StandardErrorPath": logf,
}, open(out, "wb"))
EOF
  plutil -lint -s "$new" || die "generated plist doesn't lint"
  install_plist "$new" "$MATRIX_PLIST"
  rm -f "$new"
  echo "installed $MATRIX_PLIST"
  if [ -x "$CUR/$MATRIX_BIN" ]; then
    step "start the adapter"
    start_adapter
  else
    echo "the live release has no $MATRIX_BIN; the adapter starts with the next install"
  fi
}

# ── shared: check out, compile ─────────────────────────────────────────
checkout_into() { # dir commit: a detached worktree of this repo at commit
  local src=$1 ref=$2
  mkdir -p "$(dirname "$src")"
  if [ -d "$src/.git" ] || [ -f "$src/.git" ]; then
    git -C "$src" checkout --quiet --detach "$ref"
    git -C "$src" clean -fdq
  else
    git worktree add --quiet --detach "$src" "$ref"
  fi
}

compile_into() { # src version dir: build src, copy the binary to dir
  local src=$1 ver=$2 dir=$3
  step "compile (AVATAR_VERSION=$ver)"
  (cd "$src" && export AVATAR_VERSION="$ver" CARGO_TARGET_DIR="$BUILD_DIR/target" &&
    cargo build --release --locked -p "$BIN" -p "$MATRIX_BIN")
  mkdir -p "$dir"
  cp -p "$BUILD_DIR/target/release/$BIN" "$BUILD_DIR/target/release/$MATRIX_BIN" "$dir/"
}

# ── build ──────────────────────────────────────────────────────────────
cmd_build() {
  local tag=$1 rebuild=${2:-}
  check_tag "$tag"
  load_plist
  cd "$REPO"
  git fetch --quiet --tags origin
  git rev-parse -q --verify "refs/tags/$tag" >/dev/null || die "no tag $tag"
  git ls-remote --exit-code --tags origin "refs/tags/$tag" >/dev/null ||
    die "$tag isn't on origin; publish the release first"
  git merge-base --is-ancestor "$tag" origin/main || die "$tag isn't on main"
  local dest="$REL/$tag"
  if [ -e "$dest" ]; then
    [ "$(live_tag)" != "$tag" ] || die "$tag is live; nothing to build"
    [ "$rebuild" = --rebuild ] || die "$dest exists; pass --rebuild to replace it"
    rm -rf "$dest"
  fi

  local src="$BUILD_DIR/src"
  step "check out $tag in $src"
  checkout_into "$src" "$tag"
  local sha
  sha=$(git -C "$src" rev-parse HEAD)

  local part="$REL/.$tag.partial"
  rm -rf "$part"
  compile_into "$src" "$tag" "$part"
  step "stage $dest"
  printf 'tag %s\ncommit %s\nbuilt %s\n' "$tag" "$sha" "$(date '+%F %T %z')" >"$part/BUILD"
  mv "$part" "$dest"
  echo "built $tag ($sha): $dest"
  echo "next, in a terminal: scripts/deploy.sh install $tag"
}

# ── shared: stop, start, checks ────────────────────────────────────────
loaded() { $SUDO "$LAUNCHCTL" print "system/${1:-$LABEL}" >/dev/null 2>&1; }

svc_stop() { # label prog name
  if ! loaded "$1"; then
    echo "$3 not loaded; nothing to stop"
    return 0
  fi
  $SUDO "$LAUNCHCTL" bootout "system/$1" || die "bootout $1 failed"
  local _
  for _ in $(seq 1 30); do
    pgrep -f "$2" >/dev/null || break
    sleep 1
  done
  ! pgrep -f "$2" >/dev/null || die "$3 still running after bootout"
  echo "$3 stopped"
}

svc_start() { # plist log up-line: bootstrap, wait for up-line; 1 if it never came
  local before=0 up=0 _
  [ -z "$2" ] || [ ! -f "$2" ] || before=$(wc -l <"$2" | tr -d ' ')
  $SUDO "$LAUNCHCTL" bootstrap system "$1" || die "bootstrap $1 failed"
  [ -n "$2" ] || return 0
  for _ in $(seq 1 60); do
    if tail -n +"$((before + 1))" "$2" 2>/dev/null | grep -q "$3"; then
      up=1
      break
    fi
    sleep 1
  done
  echo "new log lines:"
  tail -n +"$((before + 1))" "$2" 2>/dev/null | tail -15 | sed 's/^/   /'
  [ "$up" = 1 ]
}

stop_server() {
  STOPPED=1
  svc_stop "$LABEL" "$PROG" server
}

log_lines() { # lines in the server log so far
  [ -n "$LOG" ] && [ -f "$LOG" ] && wc -l <"$LOG" | tr -d ' ' || echo 0
}

start_server() {
  STOPPED=1
  svc_start "$PLIST" "$LOG" "$UP_LINE" || die "no '$UP_LINE' in $LOG within 60s"
  STOPPED=0
}

# The Matrix adapter, once setup-matrix installed its plist. It follows
# the server: stopped first, started after, from the same release. Its
# trouble is a warning: the display works without it.
has_adapter() { [ -r "$MATRIX_PLIST" ]; }

stop_adapter() {
  has_adapter || return 0
  svc_stop "$(pb Label "$MATRIX_PLIST")" "$(pb ProgramArguments:0 "$MATRIX_PLIST")" adapter
}

start_adapter() {
  has_adapter || return 0
  if [ ! -x "$CUR/$MATRIX_BIN" ]; then
    echo "warning: $(live_tag) has no $MATRIX_BIN; the adapter stays stopped" >&2
    return 0
  fi
  local log
  log=$(pb StandardOutPath "$MATRIX_PLIST")
  svc_start "$MATRIX_PLIST" "$log" "$MATRIX_UP" ||
    echo "warning: no '$MATRIX_UP' in $log within 60s; check that log" >&2
}

mark_started() { # tag log-line: what verify reads the log from
  printf 'started %s\nlog_line %s\n' "$(date '+%F %T %z')" "$2" >"$REL/$1/INSTALLED"
}

http_get() { # name path -> $TMP/name.out, prints the status code
  curl -s -o "$TMP/$1.out" -w '%{http_code}' "http://$LISTEN$2"
}

run_checks() { # tag [check-script]
  local tag=$1 extra=${2:-} path code name
  step "checks"
  TMP=$(mktemp -d "${TMPDIR:-/tmp}/avatar-deploy.XXXXXX")
  while read -r name path; do
    code=$(http_get "$name" "$path")
    echo "GET $path -> $code"
    [ "$code" = 200 ] || die "$path returned $code"
  done <<'EOF'
healthz /healthz
page /
state /state
screens /screens
status /status
EOF
  echo "screens: $(/usr/bin/python3 -I -c '
import json, sys
print(", ".join(s["name"] for s in json.load(open(sys.argv[1]))) or "none")' "$TMP/screens.out")"
  # A command without the token must be refused when a token is set. No
  # body is sent, so nothing on the display changes either way.
  if [ -n "$TOKEN_FILE" ]; then
    code=$(curl -s -o /dev/null -w '%{http_code}' -X POST "http://$LISTEN/sprite")
    echo "POST /sprite without a token -> $code"
    [ "$code" = 401 ] || die "commands answer $code without the token; expected 401"
  else
    echo "note: no AVATAR_TOKEN_FILE; commands are open to anything that reaches the port"
  fi
  code=$(http_get version /version)
  [ "$code" = 200 ] || die "/version returned $code"
  local got
  got=$(/usr/bin/python3 -I -c '
import json, sys
try:
    print(json.load(open(sys.argv[1])).get("version"))
except Exception:
    print("unreadable")' "$TMP/version.out")
  echo "GET /version -> $got"
  [ "$got" = "$tag" ] || die "server reports $got, expected $tag"
  if [ -n "$extra" ]; then
    /usr/bin/python3 -I "$extra" "$TMP" || die "$extra failed"
  fi
  rm -rf "$TMP"
}

# ── try ────────────────────────────────────────────────────────────────
try_clean() { # stop a leftover try server, delete scratch data
  local pid
  if [ -f "$TRY_PIDFILE" ]; then
    pid=$(cat "$TRY_PIDFILE")
    if ps -p "$pid" -o command= 2>/dev/null | grep -q "$TRY_BIN/"; then
      kill "$pid" 2>/dev/null || true
      echo "stopped try server $pid"
    fi
    rm -f "$TRY_PIDFILE"
  fi
  if [ -e "$TRY_DATA" ]; then
    rm -rf "$TRY_DATA"
    echo "deleted $TRY_DATA"
  fi
}

TRY_PID=
try_stop() {
  trap - EXIT INT TERM
  if [ -n "$TRY_PID" ]; then
    kill "$TRY_PID" 2>/dev/null || true
    wait "$TRY_PID" 2>/dev/null || true
    echo "try server stopped; last log lines:"
    tail -5 "$TRY_DATA/server.log" 2>/dev/null | sed 's/^/   /' || true
  fi
  rm -f "$TRY_PIDFILE"
  rm -rf "$TRY_DATA"
  echo "deleted $TRY_DATA"
}

cmd_try() { # ref port check-script
  local ref=$1 port=$2 extra=$3
  [[ $port =~ ^[0-9]+$ ]] || die "--port takes a number"
  load_plist
  [ -z "$extra" ] || [ -r "$extra" ] || die "can't read $extra"
  LISTEN=127.0.0.1:$port
  ! lsof -nP -iTCP:"$port" -sTCP:LISTEN >/dev/null 2>&1 ||
    die "something already listens on $port; pick another with --port"
  cd "$REPO"
  git fetch --quiet origin || echo "note: git fetch failed; using local refs"
  local sha ver
  sha=$(git rev-parse -q --verify "$ref^{commit}") || die "no commit $ref"
  ver="try-$(git rev-parse --short "$sha")"
  try_clean

  step "check out $ref ($sha) in $TRY_SRC"
  checkout_into "$TRY_SRC" "$sha"
  rm -rf "$TRY_BIN"
  compile_into "$TRY_SRC" "$ver" "$TRY_BIN"

  (umask 077 && mkdir -p "$TRY_DATA")
  chmod 700 "$TRY_DATA"
  trap try_stop EXIT
  trap 'exit 130' INT TERM
  # A scratch token, so the try server never accepts the live one.
  (umask 077 && openssl rand -hex 32 >"$TRY_DATA/token")
  TOKEN_FILE="$TRY_DATA/token"

  step "start $ver on $LISTEN"
  local envs=("AVATAR_ADDR=$LISTEN" "AVATAR_TOKEN_FILE=$TOKEN_FILE") _ up=0
  [ -z "$SCREENS" ] || envs+=("AVATAR_SCREENS=$SCREENS")
  env -i HOME="$HOME" PATH="/usr/bin:/bin:/usr/sbin:/sbin" "${envs[@]}" \
    "$TRY_BIN/$BIN" >"$TRY_DATA/server.log" 2>&1 &
  TRY_PID=$!
  echo "$TRY_PID" >"$TRY_PIDFILE"
  for _ in $(seq 1 60); do
    kill -0 "$TRY_PID" 2>/dev/null || break
    if grep -q "$UP_LINE" "$TRY_DATA/server.log"; then up=1; break; fi
    sleep 1
  done
  sed 's/^/   /' "$TRY_DATA/server.log" | tail -15
  [ "$up" = 1 ] || die "the try server didn't come up"
  run_checks "$ver" "$extra"

  printf '\n=== %s (%s) runs on http://%s ===\n' "$ref" "$ver" "$LISTEN"
  echo "token: $TOKEN_FILE (scratch, deleted on stop)"
  echo "log:   $TRY_DATA/server.log"
  echo "stop:  Ctrl-C, or kill $$"
  wait "$TRY_PID" || true
  echo "the try server exited by itself"
}

# ── env ────────────────────────────────────────────────────────────────
plist_xml() { plutil -convert xml1 -o - "$1"; }

env_key() { # key: refuse names that aren't env vars or look like secrets
  [[ $1 =~ ^[A-Z][A-Z0-9_]*$ ]] || die "$1 isn't an env var name (A-Z, 0-9, _)"
  case $1 in
  *TOKEN | *SECRET | *PASSWORD | *KEY)
    die "$1 would put a secret in a world-readable plist; point ${1}_FILE at a 0600 file instead"
    ;;
  *_DATA_DIR | *_ADDR)
    echo "warning: $1 changes where the server keeps its releases or listens" >&2
    ;;
  esac
}

ENV_YES=0 ENV_RESTART=0 ENV_MATRIX=0
env_apply() { # new-plist: lint, diff, ask, back up, install
  local new=$1 backups="$DATA/plist-backups" b
  plutil -lint -s "$new" || die "the edited plist doesn't lint; $PLIST is unchanged"
  if cmp -s <(plist_xml "$PLIST") <(plist_xml "$new"); then
    echo "no change to $PLIST"
    return 0
  fi
  step "changes to $PLIST"
  diff -u --label "now" --label "new" <(plist_xml "$PLIST") <(plist_xml "$new") || true
  if [ "$ENV_YES" != 1 ]; then
    [ -t 0 ] || die "run this in a terminal to confirm (nothing changed)"
    local a
    read -r -p "install this plist? [y/N] " a
    [ "$a" = y ] || [ "$a" = Y ] || die "not installed; $PLIST is unchanged"
  fi
  (umask 077 && mkdir -p "$backups")
  # Named by time, so sorting by name puts the newest last.
  b="$backups/$(basename "$PLIST").$(date '+%Y%m%d-%H%M%S')"
  local base=$b n=1
  while [ -e "$b" ]; do n=$((n + 1)) && b="$base-$n"; done
  cp -p "$PLIST" "$b"
  cmp -s "$PLIST" "$b" || die "plist backup differs from the original"
  echo "backed up the live plist to $b"
  install_plist "$new" || die "install failed; the old plist is in $b"
  echo "installed $PLIST"
  n=0
  for b in $(ls "$backups" | grep -F "$(basename "$PLIST")." | sort -r); do
    n=$((n + 1))
    [ "$n" -le "$KEEP_PLIST_BACKUPS" ] || rm -f "$backups/$b"
  done
  if [ "$ENV_RESTART" = 1 ]; then
    load_plist
    [ -n "$(live_tag)" ] || die "--restart needs an installed release"
    if [ "$ENV_MATRIX" = 1 ]; then
      step "restart the adapter"
      stop_adapter
      start_adapter
      return 0
    fi
    step "restart $(live_tag)"
    stop_adapter
    stop_server
    mark_started "$(live_tag)" "$(log_lines)"
    start_server
    start_adapter
    run_checks "$(live_tag)"
  else
    echo "takes effect at the next start: install, rollback, or env --restart"
  fi
}

cmd_env() { # [--matrix] action args...
  if [ "${1:-}" = --matrix ]; then
    ENV_MATRIX=1
    shift
    PLIST=$MATRIX_PLIST
    [ -r "$PLIST" ] || die "no adapter plist $PLIST (scripts/deploy.sh setup-matrix)"
  fi
  [ $# -ge 1 ] || usage
  local action=$1
  shift
  load_plist # the adapter's plist names the data dir too
  if [ "$action" = list ]; then
    plutil -extract EnvironmentVariables json -o - "$PLIST" | /usr/bin/python3 -I -c '
import json, sys
for k, v in sorted(json.load(sys.stdin).items()):
    print(f"{k}={v}")'
    return 0
  fi
  local a args=()
  for a in "$@"; do
    case $a in
    --yes) ENV_YES=1 ;;
    --restart) ENV_RESTART=1 ;;
    *) args+=("$a") ;;
    esac
  done
  [ "$ENV_RESTART" = 0 ] || [ -t 0 ] || [ -z "$SUDO" ] ||
    die "run --restart in a terminal (sudo asks for a password)"
  local new k v
  new=$(mktemp "${TMPDIR:-/tmp}/avatar-plist.XXXXXX")
  case $action in
  set | unset)
    [ ${#args[@]} -gt 0 ] || die "usage: deploy.sh env set KEY=VALUE... | env unset KEY..."
    cp "$PLIST" "$new"
    /usr/libexec/PlistBuddy -c "Print :EnvironmentVariables" "$new" >/dev/null 2>&1 ||
      /usr/libexec/PlistBuddy -c "Add :EnvironmentVariables dict" "$new"
    for a in "${args[@]}"; do
      if [ "$action" = set ]; then
        [[ $a == *=* ]] || die "set takes KEY=VALUE, got $a"
        k=${a%%=*} v=${a#*=}
        env_key "$k"
        [ -n "$v" ] || die "$k: empty value; use env unset $k"
        [[ $v != *[\"\'\\]* ]] && [[ $v != *$'\n'* ]] ||
          die "$k: quotes, backslashes and newlines aren't supported in values"
        /usr/libexec/PlistBuddy -c "Delete :EnvironmentVariables:$k" "$new" 2>/dev/null || true
        /usr/libexec/PlistBuddy -c "Add :EnvironmentVariables:$k string $v" "$new"
        [ "$(pb "EnvironmentVariables:$k" "$new")" = "$v" ] || die "$k didn't read back as written"
      else
        [[ $a =~ ^[A-Z][A-Z0-9_]*$ ]] || die "$a isn't an env var name"
        if [ -z "$(pb "EnvironmentVariables:$a" "$new")" ]; then
          echo "$a isn't set"
        else
          /usr/libexec/PlistBuddy -c "Delete :EnvironmentVariables:$a" "$new"
        fi
      fi
    done
    ;;
  restore)
    [ ${#args[@]} -le 1 ] || die "usage: deploy.sh env restore [FILE]"
    local from=${args[0]:-}
    if [ -z "$from" ]; then
      from=$(ls "$DATA/plist-backups" 2>/dev/null | grep -F "$(basename "$PLIST")." | sort | tail -1 || true)
      [ -n "$from" ] || die "no backups in $DATA/plist-backups"
      from="$DATA/plist-backups/$from"
    fi
    [ -r "$from" ] || die "can't read $from"
    echo "restoring $from"
    cp "$from" "$new"
    ;;
  *) die "usage: deploy.sh env list|set|unset|restore ..." ;;
  esac
  env_apply "$new"
  rm -f "$new"
}

# ── install ────────────────────────────────────────────────────────────
prune() {
  local live prev keep d n=0
  live=$(live_tag)
  prev=$(previous_tag)
  keep=$((KEEP_RELEASES - 2))
  # Newest first by when they were installed or built.
  for d in $(ls -t "$REL" | grep -E "$TAG_RE"); do
    if [ "$d" = "$live" ] || [ "$d" = "$prev" ]; then continue; fi
    n=$((n + 1))
    if [ "$n" -gt "$keep" ]; then
      rm -rf "${REL:?}/$d"
      echo "pruned release $d"
    fi
  done
}

cmd_install() {
  local tag=$1 extra=${2:-}
  check_tag "$tag"
  [ -t 0 ] || [ -z "$SUDO" ] || die "run install in a terminal (sudo asks for a password)"
  load_plist
  [ "$PROG" = "$CUR/$BIN" ] || die "$PLIST runs $PROG, not $CUR/$BIN"
  local dest="$REL/$tag"
  [ -f "$dest/BUILD" ] || die "$tag isn't built; run: scripts/deploy.sh build $tag"
  [ "$(live_tag)" != "$tag" ] || die "$tag is already live"
  [ -z "$extra" ] || [ -r "$extra" ] || die "can't read $extra"
  step "install $tag (live: $(live_tag))"
  sed 's/^/   /' "$dest/BUILD"
  [ -z "$SUDO" ] || $SUDO -v

  step "stop"
  stop_adapter
  stop_server
  step "switch current to $tag"
  ln -sfn "$tag" "$CUR"
  SWITCHED=1
  echo "$(date '+%F %T') $tag" >>"$REL/history"
  mark_started "$tag" "$(log_lines)"
  step "start"
  start_server
  start_adapter
  run_checks "$tag" "$extra"
  step "prune"
  prune
  printf '\n=== %s is live ===\n' "$tag"
  echo "next: scripts/deploy.sh verify [--log RE]"
}

# ── rollback ───────────────────────────────────────────────────────────
cmd_rollback() {
  local tag=${1:-}
  load_plist
  [ -t 0 ] || [ -z "$SUDO" ] || die "run rollback in a terminal (sudo asks for a password)"
  local live
  live=$(live_tag)
  [ -n "$live" ] || die "nothing is installed yet; nothing to roll back"
  [ -n "$tag" ] || tag=$(previous_tag)
  [ -n "$tag" ] || die "no earlier release to roll back to"
  [ -d "$REL/$tag" ] || die "$tag isn't installed in $REL"
  [ "$tag" != "$live" ] || die "$tag is already live"
  step "roll back $live -> $tag"
  [ -z "$SUDO" ] || $SUDO -v
  stop_adapter
  stop_server
  ln -sfn "$tag" "$CUR"
  echo "$(date '+%F %T') $tag rollback from $live" >>"$REL/history"
  mark_started "$tag" "$(log_lines)"
  start_server
  start_adapter
  run_checks "$tag"
  printf '\n=== rolled back to %s ===\n' "$tag"
}

# ── verify ─────────────────────────────────────────────────────────────
log_since() { # line: the log from that line on, without colour codes
  tail -n +"$1" "$LOG" | sed $'s/\x1b\\[[0-9;]*m//g'
}

cmd_verify() { # extra log-re...
  local extra=$1
  shift
  load_plist
  local tag from=1 fail=0 re hit
  tag=$(live_tag)
  [ -n "$tag" ] || die "nothing is installed in releases/current yet"
  [ -z "$extra" ] || [ -r "$extra" ] || die "can't read $extra"
  step "live: $tag"
  if [ -f "$CUR/INSTALLED" ]; then
    sed 's/^/   /' "$CUR/INSTALLED"
    from=$(($(awk '$1 == "log_line" {print $2}' "$CUR/INSTALLED") + 1))
    [ "$from" -le "$(($(log_lines) + 1))" ] || from=1 # the log was rotated
  fi
  run_checks "$tag" "$extra"
  if [ $# -gt 0 ]; then
    [ -n "$LOG" ] && [ -r "$LOG" ] || die "can't read the server log ($LOG)"
    step "log since $tag started (line $from)"
  fi
  for re in "$@"; do
    hit=$(log_since "$from" | grep -E -- "$re" | tail -1 || true)
    if [ -n "$hit" ]; then
      echo "found /$re/: $hit"
    else
      echo "NOT FOUND /$re/"
      fail=1
    fi
  done
  if [ "$fail" = 1 ]; then
    echo "verify: not everything expected was observed (see NOT FOUND above)" >&2
    exit 1
  fi
  printf '\n=== %s verified ===\n' "$tag"
}

# ── status ─────────────────────────────────────────────────────────────
cmd_status() {
  load_plist
  echo "live: $(live_tag)"
  [ ! -f "$CUR/BUILD" ] || sed 's/^/   /' "$CUR/BUILD"
  local v
  v=$(curl -s "http://$LISTEN/version" || true)
  echo "server says: ${v:-no answer on $LISTEN}"
  if ! has_adapter; then
    echo "adapter: not set up"
  elif "$LAUNCHCTL" print "system/$(pb Label "$MATRIX_PLIST")" >/dev/null 2>&1; then
    # No sudo: status runs without a terminal, and print doesn't need it.
    echo "adapter: loaded"
  else
    echo "adapter: not loaded"
  fi
  if [ -d "$REL" ]; then
    echo "installed: $(ls -t "$REL" | grep -E "$TAG_RE" | tr '\n' ' ')"
    echo "rollback target: $(previous_tag)"
    [ ! -f "$REL/history" ] || { echo "history:"; tail -5 "$REL/history" | sed 's/^/   /'; }
  fi
  [ ! -d "$TRY_DATA" ] || echo "try scratch data exists: $TRY_DATA (scripts/deploy.sh try --clean)"
}

usage() {
  sed -n '4,14p' "$0" >&2
  exit 2
}

case ${1:-} in
setup)
  shift
  addr=$DEFAULT_ADDR
  while [ $# -gt 0 ]; do
    case $1 in
    --addr) [ $# -ge 2 ] || usage; addr=$2; shift ;;
    *) usage ;;
    esac
    shift
  done
  cmd_setup "$addr"
  ;;
setup-matrix)
  shift
  hs= user= room= agent=
  while [ $# -gt 0 ]; do
    [ $# -ge 2 ] || usage
    case $1 in
    --homeserver) hs=$2 ;;
    --user) user=$2 ;;
    --room) room=$2 ;;
    --agent) agent=$2 ;;
    *) usage ;;
    esac
    shift 2
  done
  cmd_setup_matrix "$hs" "$user" "$room" "$agent"
  ;;
try)
  shift
  ref= port=8077 check=
  [ "${1:-}" != --clean ] || { [ $# -eq 1 ] || usage; try_clean; exit 0; }
  while [ $# -gt 0 ]; do
    case $1 in
    --port) [ $# -ge 2 ] || usage; port=$2; shift ;;
    --check) [ $# -ge 2 ] || usage; check=$2; shift ;;
    -*) usage ;;
    *) [ -z "$ref" ] || usage; ref=$1 ;;
    esac
    shift
  done
  [ -n "$ref" ] || usage
  cmd_try "$ref" "$port" "$check"
  ;;
build)
  [ $# -ge 2 ] || die "usage: deploy.sh build <tag> [--rebuild]"
  cmd_build "$2" "${3:-}"
  ;;
install)
  [ $# -eq 2 ] || { [ $# -eq 4 ] && [ "$3" = --check ]; } ||
    die "usage: deploy.sh install <tag> [--check SCRIPT]"
  cmd_install "$2" "${4:-}"
  ;;
rollback)
  [ $# -le 2 ] || usage
  cmd_rollback "${2:-}"
  ;;
env)
  [ $# -ge 2 ] || usage
  shift
  cmd_env "$@"
  ;;
verify)
  shift
  check= res=()
  while [ $# -gt 0 ]; do
    case $1 in
    --log) [ $# -ge 2 ] || usage; res+=("$2"); shift ;;
    --check) [ $# -ge 2 ] || usage; check=$2; shift ;;
    *) usage ;;
    esac
    shift
  done
  cmd_verify "$check" ${res[@]+"${res[@]}"}
  ;;
status) cmd_status ;;
*) usage ;;
esac
