// Card `alerts`: worst first. A `crit` item makes the card claim the
// whole content area (app.js decides the claim; the card gets `.claim`).
'use strict';
(() => {
  const { el, head, fmtWhen } = Kiosk;
  const ORDER = { crit: 0, warn: 1, info: 2 };
  Kiosk.card('alerts', {
    claims: (c) => c.items.some((a) => a.level === 'crit'),
    render(card, c) {
      head(card, c.title || 'Alerts');
      const ul = el('ul', 'scroll');
      for (const a of [...c.items].sort((x, y) => ORDER[x.level] - ORDER[y.level])) {
        const li = el('li', a.level);
        li.append(el('span', 'lvl'), el('span', 't', a.title), el('span', 'since', a.since ? fmtWhen(a.since) : ''));
        if (a.detail) li.appendChild(el('span', 'detail', a.detail));
        ul.appendChild(li);
      }
      card.appendChild(ul);
      card.classList.add('fit');
    },
  });
})();
