// Card `calendar`: events grouped by day; past ones fade.
'use strict';
(() => {
  const { el, head, parseTime, dayKey, fmtDay, fmtClock } = Kiosk;
  Kiosk.card('calendar', {
    render(card, c) {
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
  });
})();
