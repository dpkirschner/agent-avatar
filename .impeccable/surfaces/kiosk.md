# Surface brief: kiosk page (`app/`)

Mode: Operate. Confirmed in shape, 2026-11-14 (ho-ru7.13).

## Job and audience

The owner at their desk, tablet propped in landscape, glancing over
while working. They talk to the agent; it answers aloud and shows
the data it is talking about. Success: state and content readable at
a glance from a desk's distance.

## Layout (owner-pinned)

- Top and bottom status lines: thin bands of small icons
  (connection, agent state, clock, alert counts). Each comes from a
  prebuilt template that sets both its items and its style.
- Avatar always on the left, content always on the right.
- Presets change only the proportions: `chat` (avatar wide, content
  narrow), `split` (about half each), `focus` (avatar a narrow
  column, content dominant). The agent picks the preset; it never
  sends geometry.
- The transcript's home is subtitles under the avatar, in every
  preset. A `transcript` card shows the history as content when the
  agent asks for it.

## Speech, motion and transcript

- While the agent speaks, the avatar's mouth moves and the subtitle
  reveals word by word, both driven by the audio clock.
- Browser voice: word-boundary events. Server audio: the server sends
  word timings with the audio; mouth openness follows the audio level.
- Without timings, the line appears whole and the mouth follows the
  level.

## Generic avatar

- The avatar is a swappable module behind a small contract:
  `state(idle|listening|thinking|talking)`, `mouth(0..1)`,
  `size(preset)`.
- One folder per avatar, chosen by name in config. Who the avatar is
  will change often; layout and speech code never depend on it. The
  current SVG sprite becomes the first module.

## Content

- Typed cards in the page's own style: ops alerts, calendar, recent
  goals, text, list, key figures, and a chart card that can highlight
  a range or point. The agent supplies the chart data (from its
  finance and health tools).
- Configured screens as an iframe card, including embeddable chart
  views that the finance and health apps will expose, chosen by name
  plus allow-listed parameters (metric, range, highlight).
- Never HTML or URLs from the agent.

## States

- Cold start: the start-tap overlay.
- Disconnected: shown in the status line, never a blank screen.
- No content: falls back to `chat`.
- Speaking while the mic is on: the mic pauses.
- Subtitles: the current line only, the last line or two kept small.
  History lives in the `transcript` card, which scrolls.
- An alert may claim the content area.

## API (settled 2026-11-14; the agent's tool integration follows this)

`POST /display` (token) replaces `/show` and `/sprite`. Each field
sent replaces that part; omitted fields keep. `{"avatar":"thinking"}`
alone is the cheap call the adapter uses for typing.

```json
{
  "preset":  "chat | split | focus",
  "avatar":  "idle | listening | thinking | talking",
  "content": [card, ...],
  "status":  {"top":    {"template": "desk", "items": {}},
              "bottom": {"template": "desk", "items": {}}}
}
```

- Up to 3 cards, stacked in the agent's order. A `screen` card or an
  alerts card with a `crit` item takes the whole content area.
- `content: []` clears the cards and falls back to `chat`. Cards sent
  without a preset select `split`.
- The server validates every card; a bad one answers `422` naming its
  index. Plain text only, never HTML. Times are ISO 8601.

| type | fields |
|---|---|
| `text` | `title?`, `body` (<=2000 chars, paragraphs on blank lines) |
| `list` | `title?`, `items[{text, detail?, mark?: done/open/warn}]` <=12 |
| `figures` | `title?`, `items[{label, value, unit?, delta?, tone?: good/bad/neutral}]` <=6 |
| `alerts` | `items[{level: crit/warn/info, title, detail?, since?}]` <=8 |
| `calendar` | `title?`, `events[{start, end?, title, where?, all_day?}]` <=10 |
| `goals` | `title?`, `items[{name, progress 0..1, current?, target?, due?}]` <=6 |
| `chart` | `title?`, `kind: line/bar`, `unit?`, `series[{name, points[[x,y]]}]` <=4 series, <=500 points; x is a time or a category; `highlight?: {from,to}` or `{at}` |
| `transcript` | `lines?` (how many recent lines, default all kept) |
| `screen` | `name`, `params?` checked against that screen's allow-list |

Screen params are declared in the screens config and passed as a
query string. A list means allowed values; `"date"` or `"int"` means
that type:

```toml
[screen.params]
metric = ["spend", "income"]
range = ["30d", "90d", "1y"]
highlight = "date"
```

Status templates are built into the page; each declares its slots
(say `desk`: `alerts`, `mail`, `next`). An unknown slot answers
`422`. Connection and clock come from the page itself.

`/say` is unchanged for the agent. With server audio, the `say` event
gains `words: [{t_ms, text}]` (ho-ru7.12). `/state` returns the whole
display.

## Open

- Visual world: the structure is built with neutral styling (branch
  `display-frames`, 2026-11-14). Next, compare two or three worlds on
  the real layout. Candidates seen so far: Picture Book Spread (flat
  cobalt, thick-outlined face) and Exposure Record (dark backlit
  sheets, magenta for live voice).
- Settled in the build: presets `chat` 62/38, `split` 50/50, `focus`
  24/76 (rows in portrait); status templates `desk` (alerts, mail,
  next) and `note` (text). Their look comes with the world.
- Not yet tried on the iPad: live speech, word-by-word subtitles,
  mouth movement.
