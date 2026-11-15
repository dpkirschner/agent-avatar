// Card `transcript`: the conversation so far, newest at the bottom.
'use strict';
(() => {
  const { el, head, fmtClock, history, HISTORY_MAX } = Kiosk;
  function fill(card) {
    const ul = card.querySelector('ul');
    ul.textContent = '';
    const lines = history.slice(-card._lines);
    if (!lines.length) ul.appendChild(el('li', 'empty-note', 'Nothing said yet.'));
    for (const l of lines) {
      const li = el('li', l.who);
      li.append(el('span', 'who', (l.who === 'you' ? 'You' : 'Agent') + ' · ' + fmtClock(l.at)), el('span', 't', l.text));
      ul.appendChild(li);
    }
    ul.scrollTop = ul.scrollHeight;
  }
  Kiosk.card('transcript', {
    render(card, c) {
      head(card, 'Transcript');
      card.appendChild(el('ul', 'scroll from-end'));
      card._lines = c.lines || HISTORY_MAX;
      fill(card);
    },
    history(card) { fill(card); Kiosk.fitCard(card); },
  });
})();
