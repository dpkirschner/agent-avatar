---
name: agent-avatar (picture-book)
description: A friendly character on the desk, with cream picture-book cards beside it on a cobalt ground.
colors:
  cobalt: "#2340b8"
  cream: "#fbf3e2"
  ink: "#17173a"
  sunflower: "#ffc83d"
  tomato: "#e2462b"
  tomato-deep: "#c03a1f"
  periwinkle-dim: "#d3daf8"
  slate-ink-dim: "#55546f"
  parchment-line: "#e6dcc6"
  leaf: "#2f7d52"
  amber: "#c0700f"
  ochre: "#a87a0a"
  face-listening: "#ffd866"
  face-thinking: "#f5b52a"
  eye-white: "#fffaf0"
  face-offline: "#b9b4a4"
typography:
  said:
    fontFamily: "Fraunces, Georgia, serif"
    fontSize: "30px"
    fontWeight: 560
    lineHeight: 1.25
    fontVariation: "\"SOFT\" 100, \"WONK\" 1"
  value:
    fontFamily: "Fraunces, Georgia, serif"
    fontSize: "44px"
    fontWeight: 700
    lineHeight: 1.05
    fontFeature: "\"tnum\", \"lnum\""
    fontVariation: "\"SOFT\" 100, \"WONK\" 1"
  title:
    fontFamily: "Fraunces, Georgia, serif"
    fontSize: "23px"
    fontWeight: 640
    fontVariation: "\"SOFT\" 100, \"WONK\" 1"
  body:
    fontFamily: "Atkinson Hyperlegible Next, system-ui, sans-serif"
    fontSize: "17px"
    fontWeight: 400
    lineHeight: 1.4
  status:
    fontFamily: "Atkinson Hyperlegible Next, system-ui, sans-serif"
    fontSize: "15px"
    fontWeight: 600
    fontFeature: "\"tnum\""
  detail:
    fontFamily: "Atkinson Hyperlegible Next, system-ui, sans-serif"
    fontSize: "15px"
    fontWeight: 400
rounded:
  dot: "50%"
  bar: "6px"
  field: "8px"
  button: "12px"
  start: "18px"
  card: "22px"
  pill: "999px"
spacing:
  gap: "18px"
  card-pad: "16px 20px 18px"
  status-h: "40px"
components:
  card:
    backgroundColor: "{colors.cream}"
    textColor: "{colors.ink}"
    rounded: "{rounded.card}"
    padding: "16px 20px 18px"
  card-claim:
    backgroundColor: "{colors.tomato-deep}"
    textColor: "{colors.cream}"
    rounded: "{rounded.card}"
    padding: "16px 20px 18px"
  offline-pill:
    backgroundColor: "{colors.tomato-deep}"
    textColor: "{colors.cream}"
    typography: "{typography.title}"
    rounded: "{rounded.pill}"
    padding: "2px 12px"
  mic:
    backgroundColor: "{colors.cream}"
    textColor: "{colors.ink}"
    rounded: "{rounded.dot}"
    size: "68px"
  mic-listening:
    backgroundColor: "{colors.sunflower}"
    textColor: "{colors.ink}"
    rounded: "{rounded.dot}"
    size: "68px"
  start-button:
    backgroundColor: "{colors.sunflower}"
    textColor: "{colors.ink}"
    typography: "{typography.title}"
    rounded: "{rounded.start}"
    padding: "22px 40px"
  panel-button:
    backgroundColor: "{colors.cream}"
    textColor: "{colors.ink}"
    rounded: "{rounded.button}"
    padding: "10px 16px"
  goal-bar:
    backgroundColor: "{colors.cream}"
    rounded: "{rounded.bar}"
    height: "12px"
---

# Design System: agent-avatar (picture-book)

This file describes the `picture-book` theme (`app/themes/picture-book/`),
the default look of the kiosk page. Structure lives in
`app/core/layout.css` and the card folders; they read only tokens, and
this theme sets them. The `plain` theme is the neutral fallback and a
check that the layout stands without a world; it is not described here.

## Overview

**Creative North Star: "The Desk Companion"**

The kiosk is a friendly character who keeps the owner company on the
desk. The sunflower face is the presence; the cream cards beside it are
the things it is holding up to show you. The world borrows from picture
books (flat cobalt ground, cream pages, a thick dark-ink outline, a soft
wonky serif for anything said aloud), but the job is companionship at a
glance, not storytelling: the face reads from two metres away, and so
does whatever it shows.

Density is low on purpose. Large type, few marks, generous gaps, and one
idea per card. Colour is flat and saturated: cobalt carries the page,
cream carries content, sunflower marks what is live or highlighted, and
tomato is kept for what demands attention. Everything is drawn with the
same ink line, so a new card looks like it came from the same hand.

The look the world rules out is dashboard grey: dim slate text, small
uppercase eyebrow labels and hairline borders on dark panels. That is the
`plain` theme's job, and it must not creep back into this one.

**Key Characteristics:**
- Flat cobalt ground, cream cards, one dark-ink outline weight (3px).
- A sunflower face that changes tint and motion with the agent's state.
- Fraunces (soft, wonky axes on) for the spoken line, titles and figures;
  Atkinson Hyperlegible Next for everything read.
- Depth only from ink: outlines on everything, solid ink drops only under
  things you can press.
- Big, few, legible from across a desk.

## Colors

A saturated three-colour picture-book palette (cobalt, cream, sunflower)
held together by one dark ink, with tomato as the alarm.

### Primary
- **Cobalt** (`cobalt`): the page ground behind everything, and the first
  chart series. Never gradiented, never tinted.
- **Sunflower** (`sunflower`): the face, the listening mic, the start
  button, progress fills, the highlighted chart bar and highlight band,
  text selection, and "hot" status items. It means *live* or *look here*.

### Secondary
- **Tomato** (`tomato`): the connection-down dot and the second series.
- **Tomato Deep** (`tomato-deep`): tomato wherever it carries text, for
  AA contrast with cream (4.9:1; plain tomato is 3.7:1): the claim card
  surface, the "Offline" pill, bad deltas on cream (`--bad-ink`).

### Neutral
- **Cream** (`cream`): card and panel surfaces, mic at rest, the spoken
  line on cobalt, the listening ring and thinking dots.
- **Ink** (`ink`): every outline, all text on cream, pupils and mouth,
  bar strokes, list bullets.
- **Periwinkle Dim** (`periwinkle-dim`): secondary text on the cobalt
  ground (status lines, the owner's last line, hints).
- **Slate Ink Dim** (`slate-ink-dim`): secondary text on cream (details,
  labels, axis text, times).
- **Parchment Line** (`parchment-line`): grid lines and rules inside
  cards.

### Status and series
- **Leaf** (`leaf`) good, **Amber** (`amber`) warning, **Ochre**
  (`ochre`) third series; Leaf is also the fourth series. Every series
  colour holds at least 3:1 on cream (ochre 3.5:1).
- Face tints per state: idle and talking Sunflower, listening
  `face-listening`, thinking `face-thinking`, offline the greyed
  `face-offline`; eye whites `eye-white`.

### Named Rules
**The Two Grounds Rule.** Text sits on cobalt or on cream, never
anywhere else. On cobalt it is Cream or Periwinkle Dim; on cream it is
Ink or Slate Ink Dim.

**The Sunflower Means Live Rule.** Sunflower marks what is live or
highlighted. One sunflower highlight per card at most; a card that is
mostly yellow has said nothing.

**The Tomato Is an Alarm Rule.** Tomato fills a surface only for a claim
card and the "Offline" pill. Elsewhere it is a dot, a delta or a series,
never decoration.

## Typography

**Display Font:** Fraunces (with Georgia, serif), variable, axes
`SOFT 100`, `WONK 1`
**Body Font:** Atkinson Hyperlegible Next (with system-ui, sans-serif)

**Character:** A soft, slightly wonky serif gives the companion a
speaking voice; a hyperlegible sans carries anything that has to be read
quickly from a distance. Both are OFL and ship in the theme's `fonts/`
folder (`font-display: block`, so the page never flashes a fallback).

### Hierarchy
- **Said** (Fraunces 560, 30px, line-height 1.25): the agent's current
  spoken line under the face, balanced, at most three lines; two thirds
  the size in the `focus` preset.
- **Value** (Fraunces 700, 44px, line-height 1.05, tabular lining
  figures): key figures on figures cards; three quarters size for claim
  card alert titles.
- **Title** (Fraunces 640, 23px, sentence case): card titles, panel
  headings (25px), the start button (26px), the agent state word in the
  status line (17px).
- **Body** (Atkinson 400, 17px, line-height 1.4): card text, list items.
- **Status** (Atkinson 600, 15px, tabular): the top and bottom status
  lines.
- **Detail** (Atkinson 400, 15px): secondary lines, labels, the owner's
  last line, calendar day headings and transcript speaker lines (both
  sentence case, 600–700 weight); axis text 13px.

### Named Rules
**The Spoken Serif Rule.** Fraunces is for what the companion says or
names: the spoken line, titles, figures, the start button. Anything read
as data or instructions is Atkinson.

**The Sentence Case Rule.** Titles are sentence case at full title size.
No small uppercase tracked labels (`--title-case: none`), and no label
smaller than the 15px detail size except axis text. The agent state
word reads "Listening", "Thinking", "Talking", "Offline".

## Layout

A full-screen fixed grid: a top status line (40px), the stage, a bottom
status line. The stage splits the face column from the content column,
and the agent picks the split by preset; the page never takes geometry
from the agent.

- **Presets (landscape columns):** default 62/38 face/content, `split`
  50/50, `focus` 24/76; no cards gives the face the whole stage. The
  split animates over 0.4s (not under reduced motion).
- **Portrait** (fallback): the same presets become rows (62/38, 50/50,
  22/78), and in `focus` the face and subtitles sit side by side.
- **Content column:** up to three cards stacked with an 18px gap, each
  sharing the height unless it is marked to fit its content. Cards that
  fit their content sit centred in the column, level with the face, so
  a lone short card never leaves a void below it.
- **Overflow:** nothing is cut off silently. A card body that doesn't
  fit fades out over its last 2.2em; list items past the cut are counted
  in a "+N more" detail line. Text cards first shrink to 15px. The
  transcript, newest last, fades at the top instead.
- **Rhythm:** one gap (18px) between cards and around the stage; card
  padding 16px 20px 18px; status lines padded 18px at the sides and
  respect the safe areas.
- **Face:** centred in its column, up to 520px square, with subtitles and
  the 68px mic below.

## Elevation & Depth

Flat. Depth comes from ink alone: every card, button, mark and the face
carry the same dark outline, and nothing is blurred. The only offset is a
solid ink drop under things you can press.

### Shadow Vocabulary
- **Press drop** (`box-shadow: 0 4px 0 0 #17173a`): the mic button.
- **Press drop, large** (`box-shadow: 0 5px 0 0 #17173a`): the start
  button.
- **Link ring** (`box-shadow: 0 0 0 2px #17173a`): the connection dot.

### Named Rules
**The Press-Only Drop Rule.** Only tappable things get an ink drop.
Cards, panels and the face stay flat with outlines; a dropped card would
read as a button.

## Shapes

Round, friendly and outlined. Cards are generously rounded (22px), the
"Offline" pill fully round, the start button 18px, panel buttons 12px, fields 8px, progress bars 6px
pills; the mic, status dots and alert dots are circles. Outlines are 3px
ink on cards, the mic, the start button and claim cards, 2.5px on panel
buttons and small marks, 2px on fields; the face line is 6px. Charts use
a 4px plot line and outlined bars. The screen card is the one exception:
no padding, a white iframe edge to edge inside the card's outline.

## Components

### Cards
Cream pages held up beside the face.
- **Corner Style:** 22px.
- **Background:** Cream, Ink text, Ink 3px outline, no drop.
- **Title:** Fraunces title, sentence case, Ink.
- **Internal Padding:** 16px 20px 18px, 10px between title and body.
- **Claim:** a critical alert takes the content area as a Tomato Deep
  card with Cream text at full opacity and the same outline; its level
  marks are Cream (`--claim-dot`). Alert levels differ by shape as well
  as colour: crit a circle, warn a diamond, info a small ring.
- **Screen:** while its page loads, or when it can't be reached, the
  card shows the screen's title and "Loading…" or "Can't reach …" on
  cream; the frame covers it once loaded.
- **Types:** text, list (ink dot bullets, Leaf ticks for done, Amber
  diamonds for warnings), figures (Value numerals with unit and delta),
  goals (12px cream bar, ink outline, Sunflower fill), chart (outlined
  bars, the highlighted bar Sunflower and the rest cream; lines 4px in
  series colours), calendar, alerts, transcript, screen (iframe).

### Mic
A round cream button under the face. 68px circle, Ink 3px outline, press
drop. Listening turns it Sunflower.

### Start button
The one tap that unlocks audio. Sunflower, Fraunces 26px, 18px corners,
Ink 3px outline, large press drop, on the cobalt overlay.

### Status lines
Top and bottom strips on the cobalt ground, no rule between them and the
stage. Atkinson 600 15px in Periwinkle Dim, line icons drawn in
currentColor, the agent's state word in Fraunces Cream, "hot" items
Sunflower, a 10px connection dot (Tomato down, Sunflower up) ringed in
Ink. When the server has been out of reach for 5s the state word
becomes an "Offline" pill: Cream on Tomato Deep, ringed in Ink.

### Settings panel
A cream card (22px, outlined) on a 95% cobalt overlay; buttons outlined
2.5px, fields white with a 2px Ink outline.

### Debug log
A diagnostic panel, off by default (Settings › Debug log): the card
ground down the right edge under the top status line, 12px monospace
lines (time, event, details), event names in good, warn or bad ink.
The stage narrows to make room. It's a tool, not part of the kiosk look.

### The face (signature)
The `blob` avatar, dressed only through `--face-*` variables: a Sunflower
body with a 6px Ink outline, eye whites, Ink pupils and mouth. Idle
blinks every 5s and breathes; listening tints lighter, widens the eyes
and pulses a cream ring; thinking tints deeper, looks up and blinks three
cream dots; talking moves the mouth with the speech. Offline (the page
sets it while the server is out of reach) it sleeps: `face-offline`
grey, eyes shut, no breathing or blinking. The mic turns Sunflower only
while the page itself is listening. Motion stops under reduced-motion.

### Page states for an unattended screen
- **Offline:** the pill, the sleeping face, and the cards at 60%
  opacity and mostly greyed (`--stale-opacity`), so old content doesn't
  pass for live.
- **Stale line:** the spoken line dims to 45% after 10s and fades out
  after 3 minutes, so no text sits on the panel for days.
- **Night:** after 10 minutes with nothing said, shown or touched, a
  cobalt-black veil (`--night-dim`, 62%) fades in over 4s; any event,
  speech or touch lifts it. A quiet timer, not a clock: the page has no
  idea of the owner's hours.

## Do's and Don'ts

### Do:
- **Do** set colour, type and surfaces through the tokens in
  `core/layout.css`; a theme overrides tokens first and adds signature
  rules only after.
- **Do** outline every new mark in Ink at the existing weights (3px
  surfaces, 2.5px small marks).
- **Do** use Fraunces with `SOFT 100, WONK 1` for spoken lines, titles
  and figures; Atkinson for everything read.
- **Do** keep text on the two grounds: Cream or Periwinkle Dim on cobalt,
  Ink or Slate Ink Dim on cream.
- **Do** check a new card at a desk's distance in all three presets and
  in portrait.

### Don't:
- **Don't** let dashboard grey back in: no dim slate text on dark
  panels, no small uppercase tracked labels, no hairline borders.
- **Don't** give cards, panels or the face an ink drop; drops are for
  things you press.
- **Don't** fill a surface with Tomato except a claim card.
- **Don't** put more than one Sunflower highlight in a card.
- **Don't** style inside the face from a theme; dress it through the
  `--face-*` variables.
