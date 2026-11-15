// Card `figures`: up to 6 key numbers with a label, unit and change.
'use strict';
(() => {
  const { el, head, fmtNum } = Kiosk;
  Kiosk.card('figures', {
    render(card, c) {
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
  });
})();
