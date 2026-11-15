// The "blob" avatar: a round SVG face. One avatar module per folder
// under app/avatars/; the page loads the one named by AVATAR_FACE.
//
// The contract every avatar module meets (app.js never looks inside):
//   window.AvatarFace = {
//     mount(el)      draw into this element (it fills the avatar column)
//     state(s)       'idle' | 'listening' | 'thinking' | 'talking' |
//                    'offline' (the display server is out of reach)
//     mouth(v)       0 (closed) .. 1 (open); called every frame while talking
//     size(preset)   'chat' | 'split' | 'focus': how much room it has
//   }
'use strict';

(() => {
  // Static markup of our own; nothing from the agent goes in here.
  const SVG = `
<svg class="blob" viewBox="0 0 240 240" aria-hidden="true">
  <circle class="ring" cx="120" cy="120" r="92"/>
  <g class="bob">
    <circle class="body" cx="120" cy="120" r="88"/>
    <g class="eyes">
      <ellipse class="eye" cx="88" cy="105" rx="18" ry="22"/>
      <circle class="pupil" cx="90" cy="108" r="8"/>
      <ellipse class="eye" cx="152" cy="105" rx="18" ry="22"/>
      <circle class="pupil" cx="154" cy="108" r="8"/>
    </g>
    <ellipse class="mouth" cx="120" cy="160" rx="22" ry="14"/>
    <g class="dots">
      <circle cx="170" cy="48" r="7"/>
      <circle cx="190" cy="34" r="8"/>
      <circle cx="212" cy="22" r="9"/>
    </g>
  </g>
</svg>`;

  let root = null;
  let mouthEl = null;

  window.AvatarFace = {
    mount(el) {
      el.innerHTML = SVG;
      root = el.querySelector('.blob');
      mouthEl = root.querySelector('.mouth');
    },
    state(s) {
      if (root) root.dataset.state = s;
    },
    mouth(v) {
      if (!mouthEl) return;
      const open = Math.max(0, Math.min(1, v));
      // CSSOM writes are allowed under style-src 'self'.
      mouthEl.style.transform = 'scaleY(' + (0.15 + 0.85 * open).toFixed(3) + ')';
    },
    size(preset) {
      if (root) root.dataset.size = preset;
    },
  };
})();
