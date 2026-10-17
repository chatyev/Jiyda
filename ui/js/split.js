// split.js — Split View: над каждой половиной — строка с сайтом и действиями
// (поменять местами, заменить, закрыть половину), между ними — разделитель.
// Никаких двух панелей инструментов: адрес наверху показывает активную половину.
// Геометрия — та же, что split_boxes в src/app.rs.
import { S } from './core.js';

(() => {
  const panes = S.$('#panes');
  let drag = null;

  function geometry(sp) {
    const fs = S.state.fullscreen;
    const top = fs ? 0 : S.cfg.chromeH;
    const w = innerWidth, gap = S.cfg.splitGap;
    const inner = Math.max(0, w - gap);
    const lw = Math.round(inner * sp.ratio);
    return { top, lw, rx: lw + gap, rw: inner - lw, gap };
  }

  function head(p, x, w, active, side) {
    const el = S.h('div', { class: 'pane-head' + (active ? ' active' : ''), style: { left: x + 'px', width: w + 'px' }, attrs: { 'data-hit': '' } },
      S.fav(p),
      S.h('span', { class: 'ellipsis', text: `${S.titleOf(p)}${p.domain ? ' · ' + p.domain : ''}` }),
      S.h('span', { class: 'pane-acts' },
        S.iconBtn('swap', S.t('Swap sides'), () => S.post({ type: 'split_swap' }), 'sm'),
        S.iconBtn('replace', S.t('Replace page'), () => S.palette('replace', { old: p.id }), 'sm'),
        S.iconBtn('x', S.t('Close this side'), () => S.post({ type: 'close', id: p.id }), 'sm')));
    el.addEventListener('mousedown', (e) => { if (!e.target.closest('button') && !active) S.post({ type: 'activate', id: p.id }); });
    el.title = side === 'left' ? S.t('Left side') : S.t('Right side');
    return el;
  }

  function draw() {
    const st = S.state;
    const sp = st.active && S.splitOf(st.active);
    if (!sp || st.pageFullscreen) { panes.hidden = true; S.fill(panes, ); return; }
    const L = S.page(sp.left), R = S.page(sp.right);
    if (!L || !R) { panes.hidden = true; return; }
    const g = geometry(drag ? { ...sp, ratio: drag.ratio } : sp);
    panes.hidden = false;
    panes.style.top = '0px';
    const showHeads = !st.fullscreen;
    const div = S.h('div', { class: 'divider' + (drag ? ' drag' : ''), style: { left: g.lw + 'px', top: (showHeads ? 0 : 0) + 'px' }, attrs: { 'data-hit': '', role: 'separator', 'aria-orientation': 'vertical', 'aria-label': S.t('Resize Split View'), tabindex: '0', title: S.t('Drag to resize. Double-click for 50/50.') } });
    div.addEventListener('pointerdown', startDrag);
    div.addEventListener('dblclick', () => S.post({ type: 'split_ratio', ratio: 0.5, done: true }));
    div.addEventListener('keydown', (e) => {
      const r = sp.ratio + (e.key === 'ArrowLeft' ? -0.05 : e.key === 'ArrowRight' ? 0.05 : 0);
      if (r !== sp.ratio) { e.preventDefault(); S.post({ type: 'split_ratio', ratio: r, done: true }); }
    });
    S.fill(panes, 
      showHeads ? head(L, 0, g.lw, st.active === L.id, 'left') : null,
      showHeads ? head(R, g.rx, g.rw, st.active === R.id, 'right') : null,
      div);
  }

  function startDrag(e) {
    const sp = S.splitOf(S.state.active);
    if (!sp || e.button !== 0) return;
    e.preventDefault();
    drag = { ratio: sp.ratio, pending: false, el: e.currentTarget };
    e.currentTarget.setPointerCapture(e.pointerId);
    e.currentTarget.classList.add('drag');
    e.currentTarget.addEventListener('pointermove', move);
    e.currentTarget.addEventListener('pointerup', end);
    e.currentTarget.addEventListener('lostpointercapture', end);
  }
  function move(e) {
    if (!drag) return;
    const inner = Math.max(1, innerWidth - S.cfg.splitGap);
    drag.ratio = Math.min(0.8, Math.max(0.2, (e.clientX - S.cfg.splitGap / 2) / inner));
    const g = geometry({ ratio: drag.ratio });
    drag.el.style.left = g.lw + 'px';
    const heads = S.$$('.pane-head', panes);
    if (heads[0]) heads[0].style.width = g.lw + 'px';
    if (heads[1]) { heads[1].style.left = g.rx + 'px'; heads[1].style.width = g.rw + 'px'; }
    if (!drag.pending) {
      drag.pending = true;
      requestAnimationFrame(() => { if (!drag) return; drag.pending = false; S.post({ type: 'split_ratio', ratio: drag.ratio }); });
    }
  }
  function end() {
    if (!drag) return;
    const r = drag.ratio;
    drag = null;
    S.post({ type: 'split_ratio', ratio: r, done: true });
  }

  S.on('render', draw);
  addEventListener('resize', draw);
})();
