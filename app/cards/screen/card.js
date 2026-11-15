// Card `screen`: a configured screen in an iframe. The server resolved
// the name (and params) to `url`; the page never builds URLs itself.
// Frames are kept by URL so a redraw doesn't reload a screen that stays.
'use strict';
(() => {
  const { el } = Kiosk;
  let frames = new Map();
  let next = new Map();
  Kiosk.card('screen', {
    claims: () => true,
    render(card, c) {
      let f = frames.get(c.url) || next.get(c.url);
      if (!f) {
        f = el('iframe');
        f.src = c.url;
      }
      f.title = c.title || c.name;
      next.set(c.url, f);
      card.appendChild(f);
    },
    settle() {
      frames = next;
      next = new Map();
    },
  });
})();
