// Status template `note`: one line of text.
// The server checks the slots (server/src/display.rs, TEMPLATES).
'use strict';
(() => {
  const { el, icon } = Kiosk;
  Kiosk.status('note', (i) => {
    if (!i.text) return [];
    const e = el('span', 'item text');
    e.append(icon('note'), el('span', '', i.text));
    return [e];
  });
})();
