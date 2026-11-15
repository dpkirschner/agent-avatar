// agent-avatar kiosk: the shared kit. Loaded first; card and status
// modules register themselves here, and app.js draws with what they
// registered. Agent data only ever reaches the DOM as textContent.
//
//   Kiosk.card(type, { render(card, data), claims?(data),
//                      mounted?(card), history?(card), settle?() })
//   Kiosk.status(template, (items) => [element | null, ...])
//
// A card's `render` fills the <article class="card card-TYPE"> it is
// given. It adds the class `fit` when the card should take only the
// height its content needs (the rest share what is left). `claims`
// returns true when this card should take the whole content area
// alone (it then gets the class `claim`). `mounted`
// runs once the card is in the page (to measure it), `history` when
// the conversation history changes, `settle` once after every redraw.
//
// Content that doesn't fit is never cut off silently: app.js runs
// `fitCard` on every card after drawing and whenever its size changes.
// A card's body goes in an element with class `scroll`; a card may set
// `data-shrink="15"` to let its type shrink down to that many pixels
// first (text does). What still doesn't fit fades at the cut, and list
// items past it are counted in a "+N more" line. A `scroll.from-end`
// body (the transcript, newest last) fades at the top instead.
'use strict';

window.Kiosk = (() => {
  const cards = {};
  const status = {};

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

  // A card's title, when it has one.
  function head(card, title) {
    if (title) card.appendChild(el('h3', '', title));
  }

  // ---------- times ----------

  // ISO 8601 from the agent: a bare date is a local day; a time without
  // an offset is local time (as the spec has Date.parse read it).
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

  function fmtNum(v) {
    if (typeof v !== 'number') return String(v);
    const abs = Math.abs(v);
    const digits = abs >= 100 ? 0 : abs >= 10 ? 1 : 2;
    return v.toLocaleString([], { maximumFractionDigits: digits });
  }

  // ---------- icons (one stroke set, drawn in CSS) ----------

  const ICONS = {
    alerts: 'M12 3 2 20h20L12 3zM12 10v4M12 17h.01',
    mail: 'M3 6h18v12H3zM3 7l9 6 9-6',
    next: 'M12 7v5l3 2M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18z',
    note: 'M4 5h16M4 12h16M4 19h10',
  };
  function icon(name) {
    const s = svg('svg', { viewBox: '0 0 24 24', 'aria-hidden': 'true' });
    s.appendChild(svg('path', { d: ICONS[name] || '' }));
    return s;
  }

  // ---------- fitting ----------

  function overflows(box) { return box.scrollHeight > box.clientHeight + 1; }

  function fitCard(card) {
    const box = card.querySelector('.scroll');
    if (!box) return;
    const old = card.querySelector(':scope > .more');
    if (old) old.remove();
    box.classList.remove('cut');
    const min = +card.dataset.shrink;
    if (min) {
      box.style.fontSize = '';
      let size = parseFloat(getComputedStyle(box).fontSize);
      while (overflows(box) && size > min) {
        size -= 1;
        box.style.fontSize = size + 'px';
      }
    }
    if (!overflows(box)) return;
    box.classList.add('cut');
    if (box.classList.contains('from-end')) return;
    const items = box.querySelectorAll('li');
    if (!items.length) return; // running text: the fade says it
    const more = el('p', 'more');
    card.appendChild(more);
    const bottom = box.scrollTop + box.clientHeight;
    let hidden = 0;
    for (const li of items) if (li.offsetTop + li.offsetHeight > bottom + 1) hidden += 1;
    if (hidden) more.textContent = '+' + hidden + ' more';
    else more.remove();
  }

  // ---------- conversation history ----------

  // Recent lines, both sides, kept in the page only: a reload starts it
  // fresh. app.js adds to it; the transcript card reads it.
  const HISTORY_MAX = 50;
  const history = [];

  return {
    cards,
    templates: status,
    card(type, spec) { cards[type] = spec; },
    status(name, make) { status[name] = make; },
    el, svg, head, icon, fitCard,
    parseTime, isTime, fmtClock, dayKey, fmtDay, fmtWhen, fmtNum,
    HISTORY_MAX, history,
  };
})();
