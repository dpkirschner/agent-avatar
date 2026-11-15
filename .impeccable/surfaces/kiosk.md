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
  narrow, holds the transcript), `split` (about half each), `focus`
  (avatar a narrow column, content dominant). The agent picks the
  preset; it never sends geometry.

## Speech, motion and transcript

- While the agent speaks, the avatar's mouth moves and the transcript
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
- Long transcripts: scroll, with only the latest line large.
- An alert may claim the content area.

## API consequence (the agent's tool integration follows this)

- `show` and `sprite` merge into one call, `POST /display`:
  `{preset, content: [cards], status?}`.
- `say` gains word timings.
- Status lines take `{template, items}`.

## Open

- Visual world: build the structure with neutral styling first, then
  compare two or three worlds on the real layout. Candidates seen so
  far: Picture Book Spread (flat cobalt, thick-outlined face) and
  Exposure Record (dark backlit sheets, magenta for live voice).
- Exact card schemas, the preset proportions and the status
  templates are decided in the build.
