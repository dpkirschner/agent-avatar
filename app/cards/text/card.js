// Card `text`: a title and paragraphs (split on blank lines).
'use strict';
(() => {
  const { el, head } = Kiosk;
  Kiosk.card('text', {
    render(card, c) {
      head(card, c.title);
      const body = el('div', 'scroll');
      for (const para of c.body.split(/\n\s*\n/)) body.appendChild(el('p', '', para.trim()));
      card.appendChild(body);
      card.classList.add('fit');
    },
  });
})();
