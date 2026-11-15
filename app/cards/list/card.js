// Card `list`: up to 12 lines, each marked done, open or warn.
'use strict';
(() => {
  const { el, head } = Kiosk;
  Kiosk.card('list', {
    render(card, c) {
      head(card, c.title);
      const ul = el('ul', 'scroll');
      for (const it of c.items) {
        const li = el('li', it.mark || '');
        li.appendChild(el('span', 't', it.text));
        if (it.detail) li.appendChild(el('span', 'detail', it.detail));
        ul.appendChild(li);
      }
      card.appendChild(ul);
      card.classList.add('fit');
    },
  });
})();
