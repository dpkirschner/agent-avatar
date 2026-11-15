// agent-avatar kiosk page: follows the display server over SSE, shows the
// current screen and sprite state, speaks `say` lines, and sends what the
// user says to POST /heard. Browser notes behind the choices here are in
// docs/ARCHITECTURE.md (Kiosk page).
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

// ---------- sprite state ----------

// What the server says the avatar is doing; local speech and listening
// take precedence while they last.
let serverSprite = 'idle';
let speaking = false;
let listening = false;

function render() {
  app.dataset.state = speaking ? 'talking' : listening ? 'listening' : serverSprite;
  $('#mic').textContent = listening ? 'Stop' : 'Talk';
}

const tpl = $('#sprite-tpl');
$('#avatar').appendChild(tpl.content.cloneNode(true));

function flapMouth() {
  const m = document.querySelector('.sprite .mouth');
  if (!m) return;
  m.classList.add('flap');
  setTimeout(() => m.classList.remove('flap'), 140);
}

// ---------- screens ----------

const frame = $('#screen');
function showScreen(s) {
  if (s && s.url) {
    if (frame.dataset.url !== s.url) {
      frame.dataset.url = s.url;
      frame.src = s.url;
      frame.title = s.title || s.screen;
    }
    frame.hidden = false;
    app.classList.add('has-screen');
  } else {
    frame.hidden = true;
    frame.dataset.url = '';
    frame.src = 'about:blank';
    app.classList.remove('has-screen');
  }
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
  $('#said').textContent = line.text;
  if (!unlocked) return; // shown, not spoken: no tap yet
  queue.push(line);
  if (!current) next();
}

function next() {
  current = queue.shift() || null;
  if (!current) { speaking = false; render(); resumeListening(); return; }
  pauseListening();
  speaking = true;
  render();
  $('#said').textContent = current.text;
  const line = current;
  const done = () => { if (current === line) next(); };
  if (line.audio) playAudio(line, done);
  else speakText(line.text, done);
}

function playAudio(line, done) {
  player.onended = done;
  player.onerror = () => speakText(line.text, done);
  player.src = line.audio;
  player.play().catch(() => speakText(line.text, done));
}

function speakText(text, done) {
  if (!synth) { setTimeout(done, 1500); return; }
  synth.cancel();
  const u = new SpeechSynthesisUtterance(text);
  const v = voices.find((x) => x.voiceURI === store.get('voice', ''));
  if (v) u.voice = v;
  u.rate = +store.get('rate', '1');
  // Safari sometimes never fires `end`; don't stay stuck talking.
  const guard = setTimeout(done, 4000 + text.length * 120);
  const finish = () => { clearTimeout(guard); done(); };
  u.onboundary = flapMouth;
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
    if (e.error === 'not-allowed' || e.error === 'service-not-allowed') {
      wantListening = false;
      note('Microphone or speech recognition is not allowed. Check Settings › Safari.');
    }
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
  es.onopen = () => link.classList.add('up');
  es.onerror = () => link.classList.remove('up'); // EventSource retries on its own
  const on = (name, fn) => es.addEventListener(name, (e) => {
    let data;
    try { data = JSON.parse(e.data); } catch { return; }
    fn(data);
  });
  on('show', showScreen);
  on('sprite', (d) => { serverSprite = d.state; render(); });
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
