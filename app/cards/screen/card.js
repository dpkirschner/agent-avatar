// Card `screen`: a configured screen in an iframe. The server resolved
// the name (and params) to `url`; the page never builds URLs itself.
// Frames are kept by URL so a redraw doesn't reload a screen that stays.
//
// Behind the frame sits a panel with the screen's title: "Loading" until
// the frame has loaded and a probe has reached the screen, then hidden by
// the frame. Browsers "load" a frame they couldn't reach as their own
// error page, and a cross-origin frame can't be read to tell, so an
// opaque `no-cors` request to the same URL (allowed by the page CSP's
// connect-src) checks the screen answers at all. A screen that fails
// the probe, or hasn't loaded after WAIT_MS, says it can't be reached
// and is tried again every RETRY_MS.
'use strict';
(() => {
  const { el } = Kiosk;
  const WAIT_MS = 15000;
  const RETRY_MS = 60000;
  let frames = new Map();
  let next = new Map();

  function note(f) {
    const p = f._panel;
    if (!p) return;
    const name = f.title || 'this screen';
    p.querySelector('.state').textContent = f._state === 'failed'
      ? 'Can’t reach ' + name + ' right now. Trying again every minute.'
      : 'Loading…';
    p.classList.toggle('failed', f._state === 'failed');
    f.classList.toggle('waiting', f._state !== 'loaded');
  }

  function wait(f) {
    clearTimeout(f._timer);
    f._timer = setTimeout(() => {
      if (f._state === 'loaded') return;
      f._state = 'failed';
      note(f);
      f._timer = setTimeout(() => load(f), RETRY_MS);
    }, WAIT_MS);
  }

  // Shown once the frame has loaded and the probe got an answer.
  function check(f) {
    if (f._state === 'failed' || !f._loaded || !f._reachable) return;
    clearTimeout(f._timer);
    f._state = 'loaded';
    note(f);
  }

  function probe(f) {
    if (!window.fetch) { f._reachable = true; return; }
    fetch(f._url, { mode: 'no-cors', cache: 'no-store', credentials: 'omit' })
      .then(() => { f._reachable = true; check(f); })
      .catch(() => {
        clearTimeout(f._timer);
        f._state = 'failed';
        note(f);
        f._timer = setTimeout(() => load(f), RETRY_MS);
      });
  }

  function load(f) {
    f._state = 'loading';
    f._loaded = false;
    f._reachable = false;
    note(f);
    f.src = f._url;
    probe(f);
    wait(f);
  }

  function make(url) {
    const f = el('iframe');
    f._url = url;
    f.addEventListener('load', () => { f._loaded = true; check(f); });
    load(f);
    return f;
  }

  Kiosk.card('screen', {
    claims: () => true,
    render(card, c) {
      const f = frames.get(c.url) || next.get(c.url) || make(c.url);
      f.title = c.title || c.name;
      const panel = el('div', 'screen-wait');
      panel.append(el('h3', '', f.title), el('p', 'state detail'));
      f._panel = panel;
      note(f);
      next.set(c.url, f);
      card.append(panel, f);
    },
    settle() {
      for (const [url, f] of frames) if (!next.has(url)) clearTimeout(f._timer);
      frames = next;
      next = new Map();
    },
  });
})();
