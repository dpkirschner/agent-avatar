// Card `chart`: a line or bar chart from the agent's points, with an
// optional highlighted range or point. Drawn in SVG at the card's pixel
// size so text never stretches; redrawn when the card resizes (a preset
// change, a rotation).
'use strict';
(() => {
  const { el, svg, parseTime, isTime, fmtClock, fmtNum } = Kiosk;

  Kiosk.card('chart', {
    render(card, c) {
      const h = el('h3', '', c.title || '');
      if (c.series.length > 1) {
        const legend = el('span', 'legend');
        c.series.forEach((s, i) => legend.appendChild(el('span', 's' + (i + 1), s.name)));
        h.appendChild(legend);
      }
      card.appendChild(h);
      card.appendChild(el('div', 'plot'));
      card._chart = c;
    },
    mounted: drawChart,
  });

  const chartSizes = new ResizeObserver((entries) => {
    for (const e of entries) drawChart(e.target.closest('.card-chart'));
  });

  function drawChart(card) {
    const c = card && card._chart;
    if (!c) return;
    const plot = card.querySelector('.plot');
    if (!plot._observed) { chartSizes.observe(plot); plot._observed = true; }
    const W = plot.clientWidth;
    const H = plot.clientHeight;
    plot.textContent = '';
    if (W < 40 || H < 40) return;

    // x: numbers, times, or categories (any text that isn't a time).
    const xs = c.series.flatMap((s) => s.points.map((p) => p[0]));
    const kind = xs.every((x) => typeof x === 'number') ? 'num' : xs.every(isTime) ? 'time' : 'cat';
    const xval = (x) => (kind === 'num' ? x : kind === 'time' ? parseTime(x).d.getTime() : x);
    const cats = kind === 'cat' ? [...new Set(xs)] : null;
    const bars = c.kind === 'bar';

    const ys = c.series.flatMap((s) => s.points.map((p) => p[1]));
    let y0 = Math.min(...ys);
    let y1 = Math.max(...ys);
    if (bars) { y0 = Math.min(0, y0); y1 = Math.max(0, y1); }
    if (y0 === y1) { y0 -= 1; y1 += 1; }

    const R = 12, T = 10, B = 24;
    const ih = H - T - B;
    // y: round ticks (1, 2, 2.5 or 5 × 10^n), about one per 45px, and
    // the scale runs from the first tick to the last.
    const ticks = niceTicks(y0, y1, Math.max(2, Math.min(5, Math.floor(ih / 45))));
    y0 = ticks[0];
    y1 = ticks[ticks.length - 1];
    const sy = (y) => T + ih - ((y - y0) / (y1 - y0)) * ih;

    const s = svg('svg', { width: W, height: H, viewBox: `0 0 ${W} ${H}`, role: 'img' });
    s.setAttribute('aria-label', c.title || 'Chart');
    plot.appendChild(s);

    // y labels first: the left gutter is as wide as the widest one.
    const axis = svg('g', {}, 'axis');
    s.appendChild(axis);
    const ylabels = ticks.map((v, i) => {
      const t = svg('text', { y: sy(v) + 4, 'text-anchor': 'end' });
      t.textContent = fmtNum(v) + (i === ticks.length - 1 && c.unit ? ' ' + c.unit : '');
      axis.appendChild(t);
      return t;
    });
    let widest = 0;
    for (const t of ylabels) widest = Math.max(widest, t.getComputedTextLength ? t.getComputedTextLength() : 0);
    const L = Math.ceil(widest) + 14;
    for (const t of ylabels) t.setAttribute('x', L - 8);
    const iw = W - L - R;

    // Band positions for bars and categories; a linear scale otherwise.
    let slots = null;
    if (bars || kind === 'cat') {
      slots = cats || [...new Set(xs.map(xval))].sort((a, b) => a - b);
    }
    let sx;
    let band = 0;
    if (slots) {
      band = iw / slots.length;
      const idx = new Map(slots.map((v, i) => [v, i]));
      sx = (x) => L + (idx.get(xval(x)) + 0.5) * band;
    } else {
      const vs = xs.map(xval);
      const x0 = Math.min(...vs), x1 = Math.max(...vs);
      sx = (x) => L + (x1 === x0 ? iw / 2 : ((xval(x) - x0) / (x1 - x0)) * iw);
    }

    // Grid behind everything.
    const grid = svg('g', {}, 'grid');
    for (const v of ticks) grid.appendChild(svg('line', { x1: L, x2: W - R, y1: sy(v), y2: sy(v) }));
    s.insertBefore(grid, axis);

    // Highlight behind the data.
    const hl = c.highlight;
    if (hl && hl.from !== undefined) {
      const a = sx(hl.from), b = sx(hl.to);
      if (Number.isFinite(a) && Number.isFinite(b)) {
        const lo = Math.min(a, b) - band / 2, hi = Math.max(a, b) + band / 2;
        s.appendChild(svg('rect', { x: lo, y: T, width: Math.max(2, hi - lo), height: ih }, 'hl-band'));
      }
    }
    if (hl && hl.at !== undefined && Number.isFinite(sx(hl.at))) {
      const x = sx(hl.at);
      s.appendChild(svg('line', { x1: x, x2: x, y1: T, y2: T + ih }, 'hl-line'));
    }

    // x labels: first, middle, last.
    const months = kind === 'cat' && xs.every((x) => MONTH.test(x));
    const years = months ? new Set(xs.map((x) => x.slice(0, 4))).size : 0;
    const xlabel = (x) => (kind === 'time' ? fmtTick(x) : months ? fmtMonth(x, years > 1) : String(x));
    const sorted = slots
      ? slots.map((v) => (kind === 'time' ? new Date(v).toISOString() : v))
      : [...xs].sort((a, b) => xval(a) - xval(b));
    const picks = sorted.length <= 3 ? sorted : [sorted[0], sorted[Math.floor(sorted.length / 2)], sorted[sorted.length - 1]];
    picks.forEach((x, i) => {
      const anchor = picks.length > 1 && i === 0 ? 'start' : i === picks.length - 1 && picks.length > 1 ? 'end' : 'middle';
      const t = svg('text', { x: kind === 'time' && slots ? L + (slots.indexOf(new Date(x).getTime()) + 0.5) * band : sx(x), y: H - 6, 'text-anchor': anchor });
      t.textContent = xlabel(x);
      axis.appendChild(t);
    });

    // Data.
    const inHl = (x) => {
      if (!hl) return true;
      const v = xval(x);
      if (hl.at !== undefined) return v === xval(hl.at);
      if (kind === 'cat') return true;
      const a = xval(hl.from), b = xval(hl.to);
      return v >= Math.min(a, b) && v <= Math.max(a, b);
    };
    c.series.forEach((ser, i) => {
      const cls = 's' + (i + 1);
      if (bars) {
        const n = c.series.length;
        const w = Math.max(1, (band * 0.7) / n);
        for (const [x, y] of ser.points) {
          const cx = sx(x) - (band * 0.7) / 2 + w * i;
          const top = sy(Math.max(0, y)), bot = sy(Math.min(0, y));
          s.appendChild(svg('rect', { x: cx, y: top, width: w - 1, height: Math.max(1, bot - top) }, 'bar ' + cls + (inHl(x) ? '' : ' dim')));
        }
      } else {
        const pts = [...ser.points].sort((a, b) => (kind === 'cat' ? 0 : xval(a[0]) - xval(b[0])));
        const d = pts.map(([x, y], j) => (j ? 'L' : 'M') + sx(x).toFixed(1) + ' ' + sy(y).toFixed(1)).join('');
        s.appendChild(svg('path', { d }, 'line ' + cls));
        if (hl && hl.at !== undefined) {
          const p = pts.find(([x]) => xval(x) === xval(hl.at));
          if (p) s.appendChild(svg('circle', { cx: sx(p[0]), cy: sy(p[1]), r: 5 }, 'hl-dot ' + cls));
        }
      }
    });

  }

  // Round ticks covering [lo, hi], about `count` steps.
  function niceTicks(lo, hi, count) {
    const raw = (hi - lo) / count;
    const p = 10 ** Math.floor(Math.log10(raw));
    const f = raw / p;
    const step = (f <= 1 ? 1 : f <= 2 ? 2 : f <= 2.5 ? 2.5 : f <= 5 ? 5 : 10) * p;
    const a = Math.floor(lo / step + 1e-9), b = Math.ceil(hi / step - 1e-9);
    const out = [];
    for (let i = a; i <= b; i++) out.push(+(i * step).toPrecision(12));
    return out;
  }

  // "2026-06" → "Jun" (with the year when the axis spans more than one).
  const MONTH = /^\d{4}-(0[1-9]|1[0-2])$/;
  function fmtMonth(x, withYear) {
    const d = new Date(+x.slice(0, 4), +x.slice(5, 7) - 1, 1);
    return d.toLocaleDateString([], withYear ? { month: 'short', year: '2-digit' } : { month: 'short' });
  }

  function fmtTick(x) {
    const t = parseTime(x);
    if (!t) return String(x);
    return t.dateOnly || (t.d.getHours() === 0 && t.d.getMinutes() === 0)
      ? t.d.toLocaleDateString([], { month: 'short', day: 'numeric' })
      : fmtClock(t.d);
  }

})();
