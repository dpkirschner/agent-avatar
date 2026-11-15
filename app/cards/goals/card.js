// Card `goals`: up to 6 goals with a progress bar.
'use strict';
(() => {
  const { el, head, fmtNum, fmtWhen } = Kiosk;
  Kiosk.card('goals', {
    render(card, c) {
      head(card, c.title || 'Goals');
      const ul = el('ul', 'scroll');
      for (const g of c.items) {
        const li = el('li', 'goal');
        const h = el('div', 'head');
        h.appendChild(el('span', 't', g.name));
        const nums = [];
        if (g.current !== undefined && g.target !== undefined) nums.push(fmtNum(g.current) + ' / ' + fmtNum(g.target));
        else nums.push(Math.round(g.progress * 100) + '%');
        if (g.due) nums.push('by ' + fmtWhen(g.due));
        h.appendChild(el('span', 'nums', nums.join(' · ')));
        const bar = el('div', 'bar');
        const fill = el('div', 'fill');
        fill.style.width = (g.progress * 100).toFixed(1) + '%';
        bar.appendChild(fill);
        li.append(h, bar);
        ul.appendChild(li);
      }
      card.appendChild(ul);
      card.classList.add('fit');
    },
  });
})();
