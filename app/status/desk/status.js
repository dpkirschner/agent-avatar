// Status template `desk`: alert and mail counts, and the next event.
// The server checks the slots (server/src/display.rs, TEMPLATES).
'use strict';
(() => {
  const { el, icon } = Kiosk;
  function count(name, n, label) {
    if (n === undefined) return null;
    const e = el('span', 'item' + (n === 0 ? ' zero' : name === 'alerts' ? ' hot' : ''));
    e.title = label;
    e.append(icon(name), el('span', '', n));
    return e;
  }
  function text(name, t) {
    if (!t) return null;
    const e = el('span', 'item text');
    e.append(icon(name), el('span', '', t));
    return e;
  }
  Kiosk.status('desk', (i) => [count('alerts', i.alerts, 'Alerts'), count('mail', i.mail, 'Mail'), text('next', i.next)]);
})();
