// agent-avatar kiosk page: follows the display server over SSE, draws the
// display (preset, cards, status lines) and the avatar, speaks `say` lines
// with word-by-word subtitles, and sends what the user says to POST /heard.
// Browser notes behind the choices here are in docs/ARCHITECTURE.md
// (Kiosk page). Agent data only ever reaches the DOM as textContent.
'use strict';

const $ = (sel) => document.querySelector(sel);
const app = $('#app');
const { el, fmtClock, history, HISTORY_MAX } = Kiosk;

// ---------- small helpers ----------

const store = {
  get(k, d) { try { const v = localStorage.getItem('avatar.' + k); return v === null ? d : v; } catch { return d; } },
  set(k, v) { try { localStorage.setItem('avatar.' + k, v); } catch { /* private mode */ } },
};

// The display server's token, pasted once into settings. `heard` needs it:
// what the page hears reaches the agent as if the owner said it.
let token = store.get('token', '');

// ---------- debug log ----------

// A timestamped trace of what the page did and what the browser told it:
// the event stream, each step of speech recognition (with what it thought
// it heard), POST /heard, speech out, page errors. Always kept in memory
// (the last DBG_MAX lines); Settings › Debug log shows it in a side panel.
// Agent and speech text only ever reach the DOM as textContent.
const DBG_MAX = 400;
const dbgLines = [];
const dbgBox = $('#debug-log');
let dbgShown = false;

function stamp() {
  const d = new Date();
  return d.toTimeString().slice(0, 8) + '.' + String(d.getMilliseconds()).padStart(3, '0');
}

function fmtData(data) {
  return Object.entries(data)
    .filter(([, v]) => v !== undefined && v !== null && v !== '')
    .map(([k, v]) => k + '=' + (typeof v === 'number' || typeof v === 'boolean' ? v : JSON.stringify(v)))
    .join(' ');
}

function short(text, n = 80) {
  text = String(text);
  return text.length > n ? text.slice(0, n - 1) + '…' : text;
}

function dbgRow(line) {
  const row = el('div', 'dl' + (line.level ? ' ' + line.level : ''));
  row.append(el('span', 'dt', line.t), el('span', 'de', line.ev), el('span', 'dd', line.text));
  line.row = row;
  return row;
}

function dbgFollow(fn) {
  const atEnd = dbgBox.scrollHeight - dbgBox.scrollTop - dbgBox.clientHeight < 40;
  fn();
  if (atEnd) dbgBox.scrollTop = dbgBox.scrollHeight;
}

// level: '' | 'good' | 'warn' | 'bad'
function dbg(ev, data = {}, level = '') {
  const line = { t: stamp(), ev, text: fmtData(data), level, row: null };
  dbgLines.push(line);
  const old = dbgLines.length > DBG_MAX ? dbgLines.shift() : null;
  if (!dbgShown) return;
  dbgFollow(() => {
    if (old && old.row) old.row.remove();
    dbgBox.append(dbgRow(line));
  });
}

// Partial transcripts arrive many times a second: one line that updates.
function dbgInterim(text) {
  const last = dbgLines[dbgLines.length - 1];
  if (!last || last.ev !== 'sr-interim') { dbg('sr-interim', { text }); return; }
  last.t = stamp();
  last.text = fmtData({ text });
  if (last.row) dbgFollow(() => last.row.replaceWith(dbgRow(last)));
}

function showDebug(on) {
  dbgShown = on;
  $('#debug').hidden = !on;
  app.classList.toggle('debugging', on);
  dbgBox.textContent = '';
  for (const l of dbgLines) l.row = null;
  if (on) {
    dbgBox.append(...dbgLines.map(dbgRow));
    dbgBox.scrollTop = dbgBox.scrollHeight;
  }
}

addEventListener('error', (e) => dbg('js-error', { msg: e.message, at: e.filename ? e.filename.split('/').pop() + ':' + e.lineno : '' }, 'bad'));
addEventListener('unhandledrejection', (e) => dbg('js-rejection', { msg: String(e.reason) }, 'bad'));

class Refused extends Error {}

async function postJSON(path, body) {
  const headers = { 'Content-Type': 'application/json' };
  if (token) headers.Authorization = 'Bearer ' + token;
  const res = await fetch(path, { method: 'POST', headers, body: JSON.stringify(body) });
  if (res.status === 401) throw new Refused(path);
  if (!res.ok) throw new Error(path + ' ' + res.status);
  return res.json();
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
// Starts up so a page load doesn't flash "Offline"; connect() gives the
// stream a few seconds to open before saying otherwise.
let linkUp = true;

// The state word in the status line, sentence case.
const STATE_WORDS = { idle: '', listening: 'Listening', thinking: 'Thinking', talking: 'Talking', offline: 'Offline' };

function render() {
  // Out of reach, the face sleeps unless the page itself is busy
  // (speaking a line it already has, or listening).
  const s = speaking ? 'talking' : listening ? 'listening' : linkUp ? serverState : 'offline';
  app.dataset.state = s;
  face.state(s);
  $('#mic').textContent = listening ? 'Stop' : 'Talk';
  // The mic lights for this page's own recording only, not for a
  // `listening` the agent set.
  app.classList.toggle('rec', listening);
  $('#agent-state').textContent = linkUp ? (STATE_WORDS[s] ?? s) : STATE_WORDS.offline;
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

// Both sides go into Kiosk.history; cards that show it are told.
function remember(who, text) {
  history.push({ who, text, at: new Date() });
  if (history.length > HISTORY_MAX) history.shift();
  for (const card of content.querySelectorAll('.card')) {
    const spec = Kiosk.cards[card.dataset.type];
    if (spec && spec.history) spec.history(card);
  }
}

// The line shows at most a page of lines at a time (three; five in
// `focus`: whatever #said's max-height holds). A longer line pages on
// as the words are revealed, so the word being said is always in view;
// a line revealed all at once pages through on a timer instead.
const said = $('#said');
const STALE_MS = 10000;      // then the line dims
const GONE_MS = 3 * 60000;   // then it goes, so nothing burns in
const PAGE_MS = 4500;        // a page on screen, when there's no word clock
let staleTimer = 0;
let goneTimer = 0;
let pageTimer = 0;
let subtitle = null; // { text, done, todo, n }

function showLine(text, reveal) {
  clearTimeout(staleTimer);
  clearTimeout(goneTimer);
  clearInterval(pageTimer);
  pageTimer = 0;
  said.classList.remove('stale', 'gone');
  said.textContent = '';
  const p = el('span');
  const done = el('span', 'done', reveal ? '' : text);
  const todo = el('span', 'todo', reveal ? text : '');
  p.append(done, todo);
  said.appendChild(p);
  subtitle = { text, done, todo, n: reveal ? 0 : text.length };
  said.scrollTop = 0;
  if (!reveal) pageThrough();
}
function revealTo(n) {
  if (!subtitle) return;
  const { text, done, todo } = subtitle;
  n = Math.max(0, Math.min(text.length, n));
  const jump = n === text.length && subtitle.n < n && n - subtitle.n > 40;
  subtitle.n = n;
  done.textContent = text.slice(0, n);
  todo.textContent = text.slice(n);
  // A long tail revealed at once (a voice without word events) has no
  // clock to follow: page through it instead of jumping to the end.
  if (jump) { said.scrollTop = 0; pageThrough(); } else if (!pageTimer) followReveal();
}

// Lines per page and the line height, from the layout.
function pageGeometry() {
  const lh = parseFloat(getComputedStyle(said).lineHeight) || 1;
  const per = Math.max(1, Math.round(said.clientHeight / lh));
  const lines = Math.max(1, Math.round(said.scrollHeight / lh));
  return { lh, per, pages: Math.ceil(lines / per) };
}
function showPage(i) {
  const { lh, per } = pageGeometry();
  said.scrollTop = i * per * lh;
}
// The page holding the last revealed word.
function followReveal() {
  if (!subtitle) return;
  const rects = subtitle.done.getClientRects();
  if (!rects.length) { said.scrollTop = 0; return; }
  const { lh, per } = pageGeometry();
  const top = rects[rects.length - 1].top - said.getBoundingClientRect().top + said.scrollTop;
  showPage(Math.floor(Math.round(top / lh) / per));
}
function pageThrough() {
  clearInterval(pageTimer);
  pageTimer = 0;
  const { pages } = pageGeometry();
  if (pages <= 1) return;
  let i = 0;
  showPage(0);
  pageTimer = setInterval(() => {
    i += 1;
    showPage(i);
    if (i >= pages - 1) { clearInterval(pageTimer); pageTimer = 0; }
  }, PAGE_MS);
}
// The layout changed (a preset, a rotation): find the place again.
new ResizeObserver(() => { if (subtitle && !pageTimer) followReveal(); }).observe(said);

function lineDone() {
  if (subtitle && subtitle.n < subtitle.text.length) revealTo(subtitle.text.length);
  clearTimeout(staleTimer);
  clearTimeout(goneTimer);
  staleTimer = setTimeout(() => said.classList.add('stale'), STALE_MS);
  goneTimer = setTimeout(() => said.classList.add('gone'), GONE_MS);
}
// End of the word starting at `i`.
function wordEnd(text, i) {
  const m = /\s/.exec(text.slice(i));
  return m ? i + m.index : text.length;
}

// ---------- status lines ----------

function renderBar(host, bar) {
  const items = host.querySelector('.items');
  items.textContent = '';
  const make = bar && Kiosk.templates[bar.template];
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

function renderDisplay(d) {
  app.dataset.preset = d.preset;
  face.size(d.preset);
  renderBar($('#top'), d.status && d.status.top);
  const bottom = d.status && d.status.bottom;
  $('#bottom').hidden = !bottom;
  renderBar($('#bottom'), bottom);

  let cards = d.content || [];
  // A card whose module says it claims (a screen, a critical alert)
  // takes the whole content area.
  const claim = cards.find((c) => { const s = Kiosk.cards[c.type]; return s && s.claims && s.claims(c); });
  if (claim) cards = [claim];
  app.classList.toggle('empty', cards.length === 0);

  content.textContent = '';
  const drawn = [];
  for (const c of cards) {
    const card = el('article', 'card card-' + c.type);
    card.dataset.type = c.type;
    const spec = Kiosk.cards[c.type];
    try {
      if (spec) spec.render(card, c);
      else unknownCard(card);
    } catch (e) {
      card.textContent = '';
      card.appendChild(el('p', 'empty-note', 'Could not draw this card.'));
    }
    // A claiming card fills the column instead of fitting its content.
    if (c === claim) { card.classList.remove('fit'); card.classList.add('claim'); }
    content.appendChild(card);
    drawn.push([card, spec]);
  }
  for (const [card, spec] of drawn) if (spec && spec.mounted) spec.mounted(card);
  for (const spec of Object.values(Kiosk.cards)) if (spec.settle) spec.settle();
  for (const [card] of drawn) { Kiosk.fitCard(card); cardSizes.observe(card); }
  if (subtitle && !pageTimer) followReveal();
}

// A card's room changes with the preset and the other cards: measure
// what fits again.
const cardSizes = new ResizeObserver((entries) => {
  requestAnimationFrame(() => { for (const e of entries) if (e.target.isConnected) Kiosk.fitCard(e.target); });
});

function unknownCard(card) {
  card.appendChild(el('p', 'empty-note', 'This page doesn’t know that card yet. Reload?'));
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
  dbg('unlock');
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
    dbg('wakelock', {}, 'good');
    wake.addEventListener('release', () => { wake = null; dbg('wakelock-released'); });
  } catch (e) { dbg('wakelock-error', { err: String(e) }, 'warn'); } // retried on the next visibility change
}
document.addEventListener('visibilitychange', () => {
  dbg('visibility', { state: document.visibilityState });
  if (document.visibilityState === 'visible' && unlocked) keepAwake();
});

function goFullscreen() {
  const el = document.documentElement;
  const f = el.requestFullscreen || el.webkitRequestFullscreen;
  if (f) Promise.resolve(f.call(el)).catch((e) => dbg('fullscreen-error', { err: String(e) }, 'warn'));
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
  const end = () => { cancelAnimationFrame(raf); dbg('audio-end'); done(); };
  const fallback = (err) => {
    cancelAnimationFrame(raf);
    dbg('audio-failed', { err, then: 'browser voice' }, 'warn');
    speakText(line.text, done);
  };
  player.onended = end;
  player.onerror = () => fallback(player.error ? 'media error ' + player.error.code : 'media error');
  player.src = line.audio;
  player.play().then(() => { dbg('audio-start', { words: words ? words.length : 0 }); if (words) follow(); })
    .catch((e) => fallback(e.name || String(e)));
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
  if (!synth) { dbg('tts-none', {}, 'warn'); revealTo(text.length); setTimeout(done, 1500); return; }
  synth.cancel();
  const u = new SpeechSynthesisUtterance(text);
  const v = voices.find((x) => x.voiceURI === store.get('voice', ''));
  if (v) u.voice = v;
  u.rate = +store.get('rate', '1');
  const asked = performance.now();
  // Safari sometimes never fires `end`; don't stay stuck talking.
  const guard = setTimeout(() => { dbg('tts-stuck', { note: 'no end event; moving on' }, 'warn'); finish(); }, 4000 + text.length * 120);
  function finish() { clearTimeout(guard); clearTimeout(whole); done(); }
  u.onstart = () => dbg('tts-start', { voice: v ? v.name : 'default', lagMs: Math.round(performance.now() - asked) });
  u.onboundary = (e) => {
    if (e.name && e.name !== 'word') return;
    pulseMouth();
    const i = e.charIndex || 0;
    revealTo(e.charLength ? i + e.charLength : wordEnd(text, i));
  };
  u.onend = () => { dbg('tts-end'); finish(); };
  u.onerror = (e) => { dbg('tts-error', { err: e.error }, 'bad'); finish(); };
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

// The microphones the browser can see. Labels show only once the page
// has mic permission; a count that changes means a headset came or went.
async function logMics(why) {
  if (!navigator.mediaDevices || !navigator.mediaDevices.enumerateDevices) return;
  try {
    const mics = (await navigator.mediaDevices.enumerateDevices()).filter((d) => d.kind === 'audioinput');
    dbg('mics', { why, n: mics.length, names: mics.map((d) => d.label || '(no label)').join(', ') }, mics.length ? '' : 'bad');
  } catch (e) {
    dbg('mics-error', { err: String(e) }, 'warn');
  }
}
if (navigator.mediaDevices && navigator.mediaDevices.addEventListener) {
  navigator.mediaDevices.addEventListener('devicechange', () => logMics('changed'));
}

function startListening() {
  if (!SR || listening) return;
  rec = new SR();
  rec.lang = navigator.language || 'en-US';
  rec.interimResults = true;
  rec.maxAlternatives = 3;
  rec.continuous = continuous;
  lastInterim = '';
  // Each step, so a silent failure shows where it stopped: no `audiostart`
  // means the mic never opened; `audiostart` with no `speechstart` means
  // it heard nothing it took for speech.
  const t0 = performance.now();
  const since = () => Math.round(performance.now() - t0);
  let gotFinal = false;
  rec.onstart = () => { listening = true; render(); dbg('sr-start', { lang: rec.lang, continuous }, 'good'); logMics('listening'); };
  rec.onaudiostart = () => dbg('sr-audiostart', { ms: since() });
  rec.onsoundstart = () => dbg('sr-soundstart', { ms: since() });
  rec.onspeechstart = () => dbg('sr-speechstart', { ms: since() }, 'good');
  rec.onspeechend = () => dbg('sr-speechend', { ms: since() });
  rec.onsoundend = () => dbg('sr-soundend', { ms: since() });
  rec.onaudioend = () => dbg('sr-audioend', { ms: since() });
  rec.onnomatch = () => dbg('sr-nomatch', {}, 'warn');
  rec.onresult = (e) => {
    let interim = '';
    for (let i = e.resultIndex; i < e.results.length; i++) {
      const r = e.results[i];
      if (r.isFinal) {
        gotFinal = true;
        const alts = Array.from(r, (a) => a.transcript.trim() + ' (' + (a.confidence || 0).toFixed(2) + ')');
        dbg('sr-final', { text: r[0].transcript.trim(), alts: alts.join(' | ') }, 'good');
        finish(r[0].transcript);
      } else interim += r[0].transcript;
    }
    if (interim) { lastInterim = interim; dbgInterim(interim); }
    $('#interim').textContent = interim;
  };
  rec.onerror = (e) => {
    dbg('sr-error', { err: e.error, msg: e.message, ms: since() }, e.error === 'aborted' || e.error === 'no-speech' ? 'warn' : 'bad');
    // `aborted` is us (pauseListening); `no-speech` just ends the turn.
    if (e.error === 'aborted' || e.error === 'no-speech') return;
    wantListening = false; // don't let continuous mode retry a failure
    note(RECOGNITION_ERRORS[e.error] || 'Speech recognition failed (' + e.error + ').');
  };
  rec.onend = () => {
    listening = false;
    dbg('sr-end', { ms: since(), gotFinal, lastInterim: lastInterim.trim() }, gotFinal || lastInterim.trim() ? '' : 'warn');
    // Safari often never marks a result final: use the last interim.
    if (lastInterim.trim()) { dbg('sr-final-from-interim', { text: lastInterim.trim() }, 'warn'); finish(lastInterim); }
    $('#interim').textContent = '';
    render();
    // iOS ends each session after one utterance; restart to keep going.
    if (continuous && wantListening && !speaking) startListening();
  };
  try { rec.start(); } catch (e) { dbg('sr-start-error', { err: String(e) }, 'bad'); listening = false; render(); }
}

function stopListening() {
  wantListening = false;
  if (rec && listening) { dbg('sr-stop'); rec.stop(); }
}

function pauseListening() {
  if (rec && listening) {
    resumeAfterSpeech = continuous && wantListening;
    wantListening = false;
    dbg('sr-abort', { why: 'avatar speaking' });
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
  const t0 = performance.now();
  dbg('heard-send', { text: short(text) });
  try {
    const r = await postJSON('heard', { text });
    const ms = Math.round(performance.now() - t0);
    if (!r.delivered_to) {
      dbg('heard-undelivered', { ms, note: 'no adapter listening' }, 'warn');
      note('“' + text + '”: no agent is listening right now.');
    } else dbg('heard-ok', { ms, delivered_to: r.delivered_to }, 'good');
  } catch (e) {
    if (e instanceof Refused) {
      dbg('heard-refused', { status: 401, token: token ? 'wrong' : 'missing' }, 'bad');
      note('“' + text + '”: the display server wants its token. Paste it in Settings.');
      openSettings(true);
    } else {
      dbg('heard-failed', { err: e.message }, 'bad');
      note('“' + text + '”: could not reach the display server.');
    }
  }
}

$('#mic').addEventListener('click', () => {
  dbg('mic-tap', { listening, speaking });
  unlock();
  if (listening) { stopListening(); return; }
  if (speaking) stopSpeaking();
  wantListening = continuous;
  startListening();
});

// ---------- events from the display server ----------

// A dropped stream comes back within seconds (the server sends
// `retry: 2000`); only a link down for longer than LINK_GRACE_MS shows
// as offline.
const LINK_GRACE_MS = 5000;
let downTimer = 0; // pending while the link is down but not yet shown so
function setLink(up) {
  $('#link').classList.toggle('up', up);
  if (up) {
    clearTimeout(downTimer);
    downTimer = 0;
    linkUp = true;
    render();
    return;
  }
  if (!linkUp || downTimer) return; // already down, or already counting
  downTimer = setTimeout(() => { downTimer = 0; linkUp = false; render(); }, LINK_GRACE_MS);
}

function connect() {
  const es = new EventSource('events');
  setLink(false); // until it opens
  es.onopen = () => { dbg('sse-open', {}, 'good'); setLink(true); };
  // EventSource retries on its own; the status line says we're offline.
  es.onerror = () => {
    dbg('sse-error', { state: es.readyState === EventSource.CLOSED ? 'closed' : 'reconnecting' }, 'warn');
    setLink(false);
  };
  const on = (name, fn) => es.addEventListener(name, (e) => {
    let data;
    try { data = JSON.parse(e.data); } catch { dbg('sse-bad', { event: name, data: short(e.data) }, 'bad'); return; }
    fn(data);
  });
  on('display', (d) => {
    dbg('display', { preset: d.preset, cards: Array.isArray(d.content) ? d.content.map((c) => c && c.type).join(',') : '' });
    stir(); renderDisplay(d);
  });
  on('avatar', (d) => { dbg('avatar', { state: d.state }); stir(); serverState = d.state; render(); });
  on('say', (d) => { dbg('say', { text: short(d.text || ''), audio: !!d.audio }); stir(); enqueue(d); });
}
connect();

// ---------- night ----------

// After QUIET_MS with nothing said, shown or touched, a veil dims the
// screen (#app.night; --night-dim) to spare an always-on panel. Any
// event, speech or touch lifts it.
const QUIET_MS = 10 * 60000;
let quietTimer = 0;
function stir() {
  app.classList.remove('night');
  clearTimeout(quietTimer);
  quietTimer = setTimeout(() => {
    if (speaking || listening) stir();
    else app.classList.add('night');
  }, QUIET_MS);
}
document.addEventListener('pointerdown', stir, true);
stir();

// The browser chrome (theme-color) matches the theme's ground.
{
  const meta = document.querySelector('meta[name=theme-color]');
  const bg = getComputedStyle(document.body).backgroundColor;
  if (meta && bg) meta.content = bg;
}

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

$('#debug-on').checked = store.get('debug', '0') === '1';
$('#debug-on').addEventListener('change', (e) => {
  store.set('debug', e.target.checked ? '1' : '0');
  showDebug(e.target.checked);
});
$('#debug-close').addEventListener('click', () => {
  store.set('debug', '0');
  $('#debug-on').checked = false;
  showDebug(false);
});
$('#debug-clear').addEventListener('click', () => { dbgLines.length = 0; dbgBox.textContent = ''; });
$('#debug-copy').addEventListener('click', () => {
  const text = dbgLines.map((l) => l.t + ' ' + l.ev + (l.text ? ' ' + l.text : '')).join('\n');
  const b = $('#debug-copy');
  const say = (word) => { b.textContent = word; setTimeout(() => { b.textContent = 'Copy'; }, 1500); };
  if (!navigator.clipboard) { say('No clipboard'); return; }
  navigator.clipboard.writeText(text).then(() => say('Copied'), () => say('Not allowed'));
});
showDebug($('#debug-on').checked);

dbg('load', {
  sr: !!SR, tts: !!synth, secure: isSecureContext,
  token: token ? 'set' : 'missing', ua: navigator.userAgent,
});
render();
