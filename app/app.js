// agent-avatar kiosk page: follows the display server over SSE, draws the
// display (preset, cards, status lines) and the avatar, speaks `say` lines
// with word-by-word subtitles, and sends what the user says to POST /heard.
// Browser notes behind the choices here are in docs/ARCHITECTURE.md
// (Kiosk page). Agent data only ever reaches the DOM as textContent.
'use strict';

const $ = (sel) => document.querySelector(sel);
const app = $('#app');

// ---------- small helpers ----------

const store = {
  get(k, d) { try { const v = localStorage.getItem('avatar.' + k); return v === null ? d : v; } catch { return d; } },
  set(k, v) { try { localStorage.setItem('avatar.' + k, v); } catch { /* private mode */ } },
};

// The display server's token, pasted once into settings. `heard` needs it:
// what the page hears reaches the agent as if the owner said it.
let token = store.get('token', '');

class Refused extends Error {}

async function postJSON(path, body) {
  const headers = { 'Content-Type': 'application/json' };
  if (token) headers.Authorization = 'Bearer ' + token;
  const res = await fetch(path, { method: 'POST', headers, body: JSON.stringify(body) });
  if (res.status === 401) throw new Refused(path);
  if (!res.ok) throw new Error(path + ' ' + res.status);
  return res.json();
}

function el(tag, cls, text) {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text !== undefined && text !== null) e.textContent = String(text);
  return e;
}

const SVGNS = 'http://www.w3.org/2000/svg';
function svg(tag, attrs, cls) {
  const e = document.createElementNS(SVGNS, tag);
  for (const k in attrs) e.setAttribute(k, attrs[k]);
  if (cls) e.setAttribute('class', cls);
  return e;
}

// ---------- times ----------

// ISO 8601 from the agent: a bare date is a local day; a time without an
// offset is local time (as the spec has Date.parse read it).
function parseTime(s) {
  if (typeof s !== 'string') return null;
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(s);
  if (m) return { d: new Date(+m[1], +m[2] - 1, +m[3]), dateOnly: true };
  const t = Date.parse(s);
  return Number.isNaN(t) ? null : { d: new Date(t), dateOnly: false };
}
const isTime = (s) => typeof s === 'string' && /^\d{4}-\d{2}-\d{2}/.test(s) && parseTime(s) !== null;
const fmtClock = (d) => d.toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' });
function dayKey(d) { return d.getFullYear() + '-' + d.getMonth() + '-' + d.getDate(); }
function fmtDay(d) {
  const today = new Date();
  const tomorrow = new Date(today.getFullYear(), today.getMonth(), today.getDate() + 1);
  if (dayKey(d) === dayKey(today)) return 'Today';
  if (dayKey(d) === dayKey(tomorrow)) return 'Tomorrow';
  return d.toLocaleDateString([], { weekday: 'short', month: 'short', day: 'numeric' });
}
function fmtWhen(s) {
  const t = parseTime(s);
  if (!t) return s;
  if (t.dateOnly) return fmtDay(t.d);
  return dayKey(t.d) === dayKey(new Date()) ? fmtClock(t.d) : fmtDay(t.d) + ' ' + fmtClock(t.d);
}

// ---------- avatar ----------

// The avatar module (app/avatars/<name>/avatar.js) behind its contract:
// mount, state, mouth, size. A missing module leaves the column empty.
const face = window.AvatarFace || { mount() {}, state() {}, mouth() {}, size() {} };
face.mount($('#avatar'));

// What the server says the avatar is doing; local speech and listening
// take precedence while they last.
let serverState = 'idle';
let speaking = false;
let listening = false;
let linkUp = false;

function render() {
  const s = speaking ? 'talking' : listening ? 'listening' : serverState;
  app.dataset.state = s;
  face.state(s);
  $('#mic').textContent = listening ? 'Stop' : 'Talk';
  $('#agent-state').textContent = linkUp ? (s === 'idle' ? '' : s) : 'offline';
  app.classList.toggle('down', !linkUp);
}

// Mouth: a word boundary opens it, then it settles. When no boundaries
// come (some voices send none), it moves on a gentle loop instead.
let lastBoundary = 0;
let mouthLoop = 0;
function pulseMouth() { lastBoundary = performance.now(); }
function driveMouth() {
  cancelAnimationFrame(mouthLoop);
  const tick = (now) => {
    if (!speaking) { face.mouth(0); return; }
    let v;
    const since = now - lastBoundary;
    if (since < 400) v = Math.max(0.2, 1 - since / 220);
    else if (lastBoundary === 0 || since > 900) v = 0.25 + 0.35 * Math.abs(Math.sin(now / 90));
    else v = 0.2;
    face.mouth(v);
    mouthLoop = requestAnimationFrame(tick);
  };
  mouthLoop = requestAnimationFrame(tick);
}

// ---------- subtitles and history ----------

// Recent lines, both sides, for the transcript card. Kept in the page
// only: a reload starts it fresh.
const HISTORY_MAX = 50;
const history = [];
function remember(who, text) {
  history.push({ who, text, at: new Date() });
  if (history.length > HISTORY_MAX) history.shift();
  for (const c of document.querySelectorAll('.card-transcript')) fillTranscript(c);
}

const said = $('#said');
let staleTimer = 0;
let subtitle = null; // { text, done, todo }

function showLine(text, reveal) {
  clearTimeout(staleTimer);
  said.classList.remove('stale');
  said.textContent = '';
  const p = el('span');
  const done = el('span', 'done', reveal ? '' : text);
  const todo = el('span', 'todo', reveal ? text : '');
  p.append(done, todo);
  said.appendChild(p);
  subtitle = { text, done, todo };
}
function revealTo(n) {
  if (!subtitle) return;
  const { text, done, todo } = subtitle;
  n = Math.max(0, Math.min(text.length, n));
  done.textContent = text.slice(0, n);
  todo.textContent = text.slice(n);
}
function lineDone() {
  if (subtitle) revealTo(subtitle.text.length);
  staleTimer = setTimeout(() => said.classList.add('stale'), 10000);
}
// End of the word starting at `i`.
function wordEnd(text, i) {
  const m = /\s/.exec(text.slice(i));
  return m ? i + m.index : text.length;
}

// ---------- status lines ----------

const ICONS = {
  alerts: 'M12 3 2 20h20L12 3zM12 10v4M12 17h.01',
  mail: 'M3 6h18v12H3zM3 7l9 6 9-6',
  next: 'M12 7v5l3 2M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18z',
  note: 'M4 5h16M4 12h16M4 19h10',
};
function icon(name) {
  const s = svg('svg', { viewBox: '0 0 24 24', 'aria-hidden': 'true' });
  s.appendChild(svg('path', { d: ICONS[name] }));
  return s;
}
function countItem(name, n, label) {
  if (n === undefined) return null;
  const e = el('span', 'item' + (n === 0 ? ' zero' : name === 'alerts' ? ' hot' : ''));
  e.title = label;
  e.append(icon(name), el('span', '', n));
  return e;
}
function textItem(name, t) {
  if (!t) return null;
  const e = el('span', 'item text');
  e.append(icon(name), el('span', '', t));
  return e;
}
// Each template's items and look. The server checks the slots
// (server/src/display.rs, TEMPLATES); keep the two lists in step.
const STATUS_TEMPLATES = {
  desk: (i) => [countItem('alerts', i.alerts, 'Alerts'), countItem('mail', i.mail, 'Mail'), textItem('next', i.next)],
  note: (i) => [textItem('note', i.text)],
};
function renderBar(host, bar) {
  const items = host.querySelector('.items');
  items.textContent = '';
  const make = bar && STATUS_TEMPLATES[bar.template];
  if (make) for (const e of make(bar.items || {})) if (e) items.appendChild(e);
}

function tickClock() {
  const c = $('#clock');
  const now = new Date();
  c.textContent = fmtClock(now);
  c.dateTime = now.toISOString();
}
tickClock();
setInterval(tickClock, 15000);

// ---------- display ----------

const content = $('#content');
// Screen iframes by URL, so a redraw doesn't reload a page that stays.
let frames = new Map();

function renderDisplay(d) {
  app.dataset.preset = d.preset;
  face.size(d.preset);
  renderBar($('#top'), d.status && d.status.top);
  const bottom = d.status && d.status.bottom;
  $('#bottom').hidden = !bottom;
  renderBar($('#bottom'), bottom);

  let cards = d.content || [];
  // A screen or a critical alert claims the whole content area.
  const claim = cards.find((c) => c.type === 'screen' || (c.type === 'alerts' && c.items.some((a) => a.level === 'crit')));
  if (claim) cards = [claim];
  app.classList.toggle('empty', cards.length === 0);

  const keep = new Map();
  content.textContent = '';
  for (const c of cards) {
    const card = el('article', 'card card-' + c.type);
    try {
      (CARDS[c.type] || unknownCard)(card, c, keep);
    } catch (e) {
      card.textContent = '';
      card.appendChild(el('p', 'empty-note', 'Could not draw this card.'));
    }
    // A claiming card fills the column instead of fitting its content.
    if (c === claim) card.classList.remove('fit');
    if (c === claim && c.type === 'alerts') card.classList.add('claim');
    content.appendChild(card);
  }
  frames = keep;
  for (const c of content.querySelectorAll('.card-chart')) drawChart(c);
}

function head(card, title) {
  if (title) card.appendChild(el('h3', '', title));
}

function unknownCard(card) {
  card.appendChild(el('p', 'empty-note', 'This page doesn’t know that card yet. Reload?'));
}

const CARDS = {
  text(card, c) {
    head(card, c.title);
    const body = el('div', 'scroll');
    for (const para of c.body.split(/\n\s*\n/)) body.appendChild(el('p', '', para.trim()));
    card.appendChild(body);
    card.classList.add('fit');
  },

  list(card, c) {
    head(card, c.title);
    const ul = el('ul', 'scroll');
    for (const it of c.items) {
      const li = el('li', it.mark || '');
      li.appendChild(el('span', 't', it.text));
      if (it.detail) li.appendChild(el('span', 'detail', it.detail));
      ul.appendChild(li);
    }
    card.appendChild(ul);
    card.classList.add('fit');
  },

  figures(card, c) {
    head(card, c.title);
    const grid = el('div', 'grid');
    for (const f of c.items) {
      const fig = el('div', 'figure');
      fig.appendChild(el('span', 'label', f.label));
      const v = el('span', 'value', fmtNum(f.value));
      if (f.unit) v.appendChild(el('span', 'unit', f.unit));
      fig.appendChild(v);
      if (f.delta !== undefined) fig.appendChild(el('span', 'delta ' + (f.tone || ''), fmtNum(f.delta)));
      grid.appendChild(fig);
    }
    card.appendChild(grid);
    card.classList.add('fit');
  },

  alerts(card, c) {
    head(card, c.title || 'Alerts');
    const ul = el('ul', 'scroll');
    const order = { crit: 0, warn: 1, info: 2 };
    for (const a of [...c.items].sort((x, y) => order[x.level] - order[y.level])) {
      const li = el('li', a.level);
      li.append(el('span', 'lvl'), el('span', 't', a.title), el('span', 'since', a.since ? fmtWhen(a.since) : ''));
      if (a.detail) li.appendChild(el('span', 'detail', a.detail));
      ul.appendChild(li);
    }
    card.appendChild(ul);
    card.classList.add('fit');
  },

  calendar(card, c) {
    head(card, c.title || 'Calendar');
    const box = el('div', 'scroll');
    const now = Date.now();
    let lastDay = null;
    let ul = null;
    const events = c.events
      .map((e) => ({ e, t: parseTime(e.start) }))
      .filter((x) => x.t)
      .sort((a, b) => a.t.d - b.t.d);
    for (const { e, t } of events) {
      const k = dayKey(t.d);
      if (k !== lastDay) {
        box.appendChild(el('div', 'day', fmtDay(t.d)));
        ul = el('ul');
        box.appendChild(ul);
        lastDay = k;
      }
      const end = e.end ? parseTime(e.end) : null;
      const li = el('li');
      if ((end ? end.d.getTime() : t.d.getTime()) < now && !t.dateOnly) li.classList.add('past');
      const allDay = e.all_day || t.dateOnly;
      li.append(el('span', 'when', allDay ? 'All day' : fmtClock(t.d)), el('span', 't', e.title));
      if (e.where) li.appendChild(el('span', 'detail', e.where));
      ul.appendChild(li);
    }
    card.appendChild(box);
    card.classList.add('fit');
  },

  goals(card, c) {
    head(card, c.title || 'Goals');
    const ul = el('ul', 'scroll');
    for (const g of c.items) {
      const li = el('li', 'goal');
      const h = el('div', 'head');
      h.appendChild(el('span', 't', g.name));
      const nums = [];
      if (g.current !== undefined && g.target !== undefined) nums.push(fmtNum(g.current) + ' / ' + fmtNum(g.target));
      else nums.push(Math.round(g.progress * 100) + '%');
      if (g.due) nums.push('by ' + fmtWhen(g.due));
      h.appendChild(el('span', 'nums', nums.join(' · ')));
      const bar = el('div', 'bar');
      const fill = el('div', 'fill');
      fill.style.width = (g.progress * 100).toFixed(1) + '%';
      bar.appendChild(fill);
      li.append(h, bar);
      ul.appendChild(li);
    }
    card.appendChild(ul);
    card.classList.add('fit');
  },

  chart(card, c) {
    const h = el('h3', '', c.title || '');
    if (c.series.length > 1) {
      const legend = el('span', 'legend');
      c.series.forEach((s, i) => legend.appendChild(el('span', 's' + (i + 1), s.name)));
      h.appendChild(legend);
    }
    card.appendChild(h);
    card.appendChild(el('div', 'plot'));
    card._chart = c;
  },

  transcript(card, c) {
    head(card, 'Transcript');
    card.appendChild(el('ul', 'scroll'));
    card._lines = c.lines || HISTORY_MAX;
    fillTranscript(card);
  },

  screen(card, c, keep) {
    let f = frames.get(c.url);
    if (!f) {
      f = el('iframe');
      f.src = c.url;
    }
    f.title = c.title || c.name;
    keep.set(c.url, f);
    card.appendChild(f);
  },
};

function fillTranscript(card) {
  const ul = card.querySelector('ul');
  ul.textContent = '';
  const lines = history.slice(-card._lines);
  if (!lines.length) ul.appendChild(el('li', 'empty-note', 'Nothing said yet.'));
  for (const l of lines) {
    const li = el('li', l.who);
    li.append(el('span', 'who', (l.who === 'you' ? 'You' : 'Agent') + ' · ' + fmtClock(l.at)), el('span', 't', l.text));
    ul.appendChild(li);
  }
  ul.scrollTop = ul.scrollHeight;
}

function fmtNum(v) {
  if (typeof v !== 'number') return String(v);
  const abs = Math.abs(v);
  const digits = abs >= 100 ? 0 : abs >= 10 ? 1 : 2;
  return v.toLocaleString([], { maximumFractionDigits: digits });
}

// ---------- chart ----------

// Drawn in pixels at the card's size, so text never stretches; redrawn
// when the card resizes (a preset change, a rotation).
const chartSizes = new ResizeObserver((entries) => {
  for (const e of entries) drawChart(e.target.closest('.card-chart'));
});

function drawChart(card) {
  const c = card && card._chart;
  if (!c) return;
  const plot = card.querySelector('.plot');
  if (!plot._observed) { chartSizes.observe(plot); plot._observed = true; }
  const W = plot.clientWidth;
  const H = plot.clientHeight;
  plot.textContent = '';
  if (W < 40 || H < 40) return;

  // x: numbers, times, or categories (any text that isn't a time).
  const xs = c.series.flatMap((s) => s.points.map((p) => p[0]));
  const kind = xs.every((x) => typeof x === 'number') ? 'num' : xs.every(isTime) ? 'time' : 'cat';
  const xval = (x) => (kind === 'num' ? x : kind === 'time' ? parseTime(x).d.getTime() : x);
  const cats = kind === 'cat' ? [...new Set(xs)] : null;
  const bars = c.kind === 'bar';

  const ys = c.series.flatMap((s) => s.points.map((p) => p[1]));
  let y0 = Math.min(...ys);
  let y1 = Math.max(...ys);
  if (bars) { y0 = Math.min(0, y0); y1 = Math.max(0, y1); }
  if (y0 === y1) { y0 -= 1; y1 += 1; }
  const pad = (y1 - y0) * 0.08;
  if (!(bars && y0 === 0)) y0 -= pad;
  y1 += pad;

  const L = 48, R = 12, T = 8, B = 24;
  const iw = W - L - R, ih = H - T - B;
  const sy = (y) => T + ih - ((y - y0) / (y1 - y0)) * ih;

  // Band positions for bars and categories; a linear scale otherwise.
  let slots = null;
  if (bars || kind === 'cat') {
    slots = cats || [...new Set(xs.map(xval))].sort((a, b) => a - b);
  }
  let sx;
  let band = 0;
  if (slots) {
    band = iw / slots.length;
    const idx = new Map(slots.map((v, i) => [v, i]));
    sx = (x) => L + (idx.get(xval(x)) + 0.5) * band;
  } else {
    const vs = xs.map(xval);
    const x0 = Math.min(...vs), x1 = Math.max(...vs);
    sx = (x) => L + (x1 === x0 ? iw / 2 : ((xval(x) - x0) / (x1 - x0)) * iw);
  }

  const s = svg('svg', { width: W, height: H, viewBox: `0 0 ${W} ${H}`, role: 'img' });
  s.setAttribute('aria-label', c.title || 'Chart');

  // Grid and y labels: three lines.
  const grid = svg('g', {}, 'grid');
  const axis = svg('g', {}, 'axis');
  for (let i = 0; i <= 2; i++) {
    const v = y0 + ((y1 - y0) * i) / 2;
    const y = sy(v);
    grid.appendChild(svg('line', { x1: L, x2: W - R, y1: y, y2: y }));
    const t = svg('text', { x: L - 8, y: y + 4, 'text-anchor': 'end' });
    t.textContent = fmtNum(v) + (i === 2 && c.unit ? ' ' + c.unit : '');
    axis.appendChild(t);
  }
  s.append(grid);

  // Highlight behind the data.
  const hl = c.highlight;
  if (hl && hl.from !== undefined) {
    const a = sx(hl.from), b = sx(hl.to);
    if (Number.isFinite(a) && Number.isFinite(b)) {
      const lo = Math.min(a, b) - band / 2, hi = Math.max(a, b) + band / 2;
      s.appendChild(svg('rect', { x: lo, y: T, width: Math.max(2, hi - lo), height: ih }, 'hl-band'));
    }
  }
  if (hl && hl.at !== undefined && Number.isFinite(sx(hl.at))) {
    const x = sx(hl.at);
    s.appendChild(svg('line', { x1: x, x2: x, y1: T, y2: T + ih }, 'hl-line'));
  }

  // x labels: first, middle, last.
  const xlabel = (x) => (kind === 'time' ? fmtTick(x) : String(x));
  const sorted = slots
    ? slots.map((v) => (kind === 'time' ? new Date(v).toISOString() : v))
    : [...xs].sort((a, b) => xval(a) - xval(b));
  const picks = sorted.length <= 3 ? sorted : [sorted[0], sorted[Math.floor(sorted.length / 2)], sorted[sorted.length - 1]];
  picks.forEach((x, i) => {
    const anchor = picks.length > 1 && i === 0 ? 'start' : i === picks.length - 1 && picks.length > 1 ? 'end' : 'middle';
    const t = svg('text', { x: kind === 'time' && slots ? L + (slots.indexOf(new Date(x).getTime()) + 0.5) * band : sx(x), y: H - 6, 'text-anchor': anchor });
    t.textContent = xlabel(x);
    axis.appendChild(t);
  });
  s.appendChild(axis);

  // Data.
  const inHl = (x) => {
    if (!hl) return true;
    const v = xval(x);
    if (hl.at !== undefined) return v === xval(hl.at);
    if (kind === 'cat') return true;
    const a = xval(hl.from), b = xval(hl.to);
    return v >= Math.min(a, b) && v <= Math.max(a, b);
  };
  c.series.forEach((ser, i) => {
    const cls = 's' + (i + 1);
    if (bars) {
      const n = c.series.length;
      const w = Math.max(1, (band * 0.7) / n);
      for (const [x, y] of ser.points) {
        const cx = sx(x) - (band * 0.7) / 2 + w * i;
        const top = sy(Math.max(0, y)), bot = sy(Math.min(0, y));
        s.appendChild(svg('rect', { x: cx, y: top, width: w - 1, height: Math.max(1, bot - top) }, 'bar ' + cls + (inHl(x) ? '' : ' dim')));
      }
    } else {
      const pts = [...ser.points].sort((a, b) => (kind === 'cat' ? 0 : xval(a[0]) - xval(b[0])));
      const d = pts.map(([x, y], j) => (j ? 'L' : 'M') + sx(x).toFixed(1) + ' ' + sy(y).toFixed(1)).join('');
      s.appendChild(svg('path', { d }, 'line ' + cls));
      if (hl && hl.at !== undefined) {
        const p = pts.find(([x]) => xval(x) === xval(hl.at));
        if (p) s.appendChild(svg('circle', { cx: sx(p[0]), cy: sy(p[1]), r: 5 }, 'hl-dot ' + cls));
      }
    }
  });

  plot.appendChild(s);
}

function fmtTick(x) {
  const t = parseTime(x);
  if (!t) return String(x);
  return t.dateOnly || (t.d.getHours() === 0 && t.d.getMinutes() === 0)
    ? t.d.toLocaleDateString([], { month: 'short', day: 'numeric' })
    : fmtClock(t.d);
}

// ---------- unlock: one tap per page load ----------

// iOS plays agent-started audio only after a tap has played something.
// One reused <audio> element; Web Audio can hang after a lock.
const player = new Audio();
player.playsInline = true;
const SILENT = 'data:audio/wav;base64,UklGRiQAAABXQVZFZm10IBAAAAABAAEAQB8AAEAfAAABAAgAZGF0YQAAAAA=';
const synth = window.speechSynthesis || null;
let unlocked = false;

function unlock() {
  if (unlocked) return;
  unlocked = true;
  player.src = SILENT;
  player.play().catch(() => {});
  if (synth) {
    const u = new SpeechSynthesisUtterance(' ');
    u.volume = 0;
    synth.speak(u);
  }
  keepAwake();
  $('#start').hidden = true;
}

let wake = null;
async function keepAwake() {
  if (wake || !('wakeLock' in navigator)) return;
  try {
    wake = await navigator.wakeLock.request('screen');
    wake.addEventListener('release', () => { wake = null; });
  } catch { /* not allowed now; retried on the next visibility change */ }
}
document.addEventListener('visibilitychange', () => {
  if (document.visibilityState === 'visible' && unlocked) keepAwake();
});

function goFullscreen() {
  const el = document.documentElement;
  const f = el.requestFullscreen || el.webkitRequestFullscreen;
  if (f) Promise.resolve(f.call(el)).catch(() => {});
}

$('#start-btn').addEventListener('click', () => {
  unlock();
  goFullscreen();
  if (!token) openSettings(true); // first run: ask for the token up front
});

// ---------- speech out ----------

let voices = [];
function loadVoices() {
  if (!synth) return;
  voices = synth.getVoices().filter((v) => /^en/i.test(v.lang));
  const sel = $('#voice');
  const want = store.get('voice', '');
  sel.length = 1;
  for (const v of voices) {
    const o = new Option(v.name + ' (' + v.lang + ')', v.voiceURI);
    o.selected = v.voiceURI === want;
    sel.add(o);
  }
}
if (synth) { loadVoices(); synth.addEventListener('voiceschanged', loadVoices); }

const queue = [];
let current = null;

function enqueue(line) {
  remember('agent', line.text);
  if (!unlocked) { showLine(line.text, false); lineDone(); return; } // shown, not spoken: no tap yet
  queue.push(line);
  if (!current) next();
}

function next() {
  current = queue.shift() || null;
  if (!current) { speaking = false; render(); resumeListening(); return; }
  pauseListening();
  speaking = true;
  lastBoundary = 0;
  render();
  driveMouth();
  const line = current;
  const done = () => { if (current === line) { lineDone(); next(); } };
  if (line.audio) playAudio(line, done);
  else speakText(line.text, done);
}

// Server audio: with word timings the subtitle follows the audio clock;
// without, the line shows whole.
function playAudio(line, done) {
  const words = Array.isArray(line.words) ? wordOffsets(line.text, line.words) : null;
  showLine(line.text, !!words);
  let raf = 0;
  const follow = () => {
    const ms = player.currentTime * 1000;
    let n = 0;
    for (const w of words) { if (w.t_ms <= ms) { n = w.end; } else break; }
    if (n !== follow.n) { follow.n = n; revealTo(n); pulseMouth(); }
    raf = requestAnimationFrame(follow);
  };
  const end = () => { cancelAnimationFrame(raf); done(); };
  player.onended = end;
  player.onerror = () => { cancelAnimationFrame(raf); speakText(line.text, done); };
  player.src = line.audio;
  player.play().then(() => { if (words) follow(); }).catch(() => { cancelAnimationFrame(raf); speakText(line.text, done); });
}

// Where each timed word ends in the text, matched in order.
function wordOffsets(text, words) {
  let at = 0;
  const out = [];
  for (const w of words) {
    if (typeof w.text !== 'string' || typeof w.t_ms !== 'number') continue;
    const i = text.indexOf(w.text, at);
    if (i < 0) continue;
    at = i + w.text.length;
    out.push({ t_ms: w.t_ms, end: at });
  }
  return out;
}

// Browser voice: word-boundary events reveal the subtitle. A voice that
// sends none gets the whole line after a moment.
function speakText(text, done) {
  showLine(text, true);
  const whole = setTimeout(() => { if (!lastBoundary) revealTo(text.length); }, 700);
  if (!synth) { revealTo(text.length); setTimeout(done, 1500); return; }
  synth.cancel();
  const u = new SpeechSynthesisUtterance(text);
  const v = voices.find((x) => x.voiceURI === store.get('voice', ''));
  if (v) u.voice = v;
  u.rate = +store.get('rate', '1');
  // Safari sometimes never fires `end`; don't stay stuck talking.
  const guard = setTimeout(finish, 4000 + text.length * 120);
  function finish() { clearTimeout(guard); clearTimeout(whole); done(); }
  u.onboundary = (e) => {
    if (e.name && e.name !== 'word') return;
    pulseMouth();
    const i = e.charIndex || 0;
    revealTo(e.charLength ? i + e.charLength : wordEnd(text, i));
  };
  u.onend = finish;
  u.onerror = finish;
  synth.speak(u);
}

function stopSpeaking() {
  queue.length = 0;
  current = null;
  player.onended = player.onerror = null;
  player.pause();
  if (synth) synth.cancel();
  speaking = false;
  lineDone();
  render();
}

// ---------- speech in ----------

const SR = window.SpeechRecognition || window.webkitSpeechRecognition;
let rec = null;
let continuous = store.get('continuous', '0') === '1';
let wantListening = false; // continuous mode: keep restarting
let resumeAfterSpeech = false;
let lastInterim = ''; // cleared once sent, so `end` never sends it twice

if (!SR) {
  warn('This browser has no speech recognition. On an iPad, open the page in Safari.');
} else if (/CriOS|FxiOS|EdgiOS/.test(navigator.userAgent)) {
  warn('Only Safari may use speech recognition on iOS. Open this page in Safari.');
}
const RECOGNITION_ERRORS = {
  'not-allowed': 'Microphone or speech recognition is not allowed. Check Settings › Safari.',
  'service-not-allowed': 'Microphone or speech recognition is not allowed. Check Settings › Safari.',
  // Brave and other Chromium builds without Google's speech service land here.
  network: 'This browser’s speech service isn’t available. Use Safari (or Chrome).',
  'audio-capture': 'No microphone found.',
  'language-not-supported': 'Speech recognition doesn’t support this language.',
};
function warn(msg) { const w = $('#warn'); w.textContent = msg; w.hidden = false; }

function startListening() {
  if (!SR || listening) return;
  rec = new SR();
  rec.lang = navigator.language || 'en-US';
  rec.interimResults = true;
  rec.continuous = continuous;
  lastInterim = '';
  rec.onstart = () => { listening = true; render(); };
  rec.onresult = (e) => {
    let interim = '';
    for (let i = e.resultIndex; i < e.results.length; i++) {
      const r = e.results[i];
      if (r.isFinal) finish(r[0].transcript);
      else interim += r[0].transcript;
    }
    if (interim) lastInterim = interim;
    $('#interim').textContent = interim;
  };
  rec.onerror = (e) => {
    // `aborted` is us (pauseListening); `no-speech` just ends the turn.
    if (e.error === 'aborted' || e.error === 'no-speech') return;
    wantListening = false; // don't let continuous mode retry a failure
    note(RECOGNITION_ERRORS[e.error] || 'Speech recognition failed (' + e.error + ').');
  };
  rec.onend = () => {
    listening = false;
    // Safari often never marks a result final: use the last interim.
    if (lastInterim.trim()) finish(lastInterim);
    $('#interim').textContent = '';
    render();
    // iOS ends each session after one utterance; restart to keep going.
    if (continuous && wantListening && !speaking) startListening();
  };
  try { rec.start(); } catch { listening = false; render(); }
}

function stopListening() {
  wantListening = false;
  if (rec && listening) rec.stop();
}

function pauseListening() {
  if (rec && listening) {
    resumeAfterSpeech = continuous && wantListening;
    wantListening = false;
    rec.abort(); // don't hear our own voice
  }
}

function resumeListening() {
  if (resumeAfterSpeech) {
    resumeAfterSpeech = false;
    wantListening = true;
    startListening();
  }
}

function note(msg) {
  const h = $('#heard');
  h.textContent = msg;
  h.classList.add('note');
}

async function finish(text) {
  text = text.trim();
  lastInterim = '';
  if (!text) return;
  const h = $('#heard');
  h.classList.remove('note');
  h.textContent = text;
  remember('you', text);
  if (!continuous && rec && listening) rec.stop();
  try {
    const r = await postJSON('heard', { text });
    if (!r.delivered_to) note('“' + text + '”: no agent is listening right now.');
  } catch (e) {
    if (e instanceof Refused) {
      note('“' + text + '”: the display server wants its token. Paste it in Settings.');
      openSettings(true);
    } else {
      note('“' + text + '”: could not reach the display server.');
    }
  }
}

$('#mic').addEventListener('click', () => {
  unlock();
  if (listening) { stopListening(); return; }
  if (speaking) stopSpeaking();
  wantListening = continuous;
  startListening();
});

// ---------- events from the display server ----------

function connect() {
  const es = new EventSource('events');
  const link = $('#link');
  es.onopen = () => { linkUp = true; link.classList.add('up'); render(); };
  // EventSource retries on its own; the status line says we're offline.
  es.onerror = () => { linkUp = false; link.classList.remove('up'); render(); };
  const on = (name, fn) => es.addEventListener(name, (e) => {
    let data;
    try { data = JSON.parse(e.data); } catch { return; }
    fn(data);
  });
  on('display', renderDisplay);
  on('avatar', (d) => { serverState = d.state; render(); });
  on('say', enqueue);
}
connect();

// ---------- settings ----------

const settings = $('#settings');
let holdTimer = null;
const corner = $('#corner');
function openSettings(focusToken) {
  settings.hidden = false;
  if (focusToken) $('#token').focus();
}
corner.addEventListener('pointerdown', () => { holdTimer = setTimeout(() => openSettings(false), 800); });
for (const ev of ['pointerup', 'pointerleave', 'pointercancel']) {
  corner.addEventListener(ev, () => clearTimeout(holdTimer));
}

$('#token').value = token;
$('#token').addEventListener('input', (e) => {
  token = e.target.value.trim();
  store.set('token', token);
});

$('#continuous').checked = continuous;
$('#continuous').addEventListener('change', (e) => {
  continuous = e.target.checked;
  store.set('continuous', continuous ? '1' : '0');
  if (!continuous) stopListening();
});
$('#voice').addEventListener('change', (e) => store.set('voice', e.target.value));
const rate = $('#rate');
rate.value = store.get('rate', '1');
$('#rate-v').textContent = (+rate.value).toFixed(2);
rate.addEventListener('input', () => {
  store.set('rate', rate.value);
  $('#rate-v').textContent = (+rate.value).toFixed(2);
});
$('#test-voice').addEventListener('click', () => {
  unlock();
  enqueue({ text: 'Hello. This is how I sound with the browser voice.', audio: null });
});
$('#fullscreen').addEventListener('click', goFullscreen);
$('#reload').addEventListener('click', () => location.reload());
$('#close-settings').addEventListener('click', () => { settings.hidden = true; });

render();
