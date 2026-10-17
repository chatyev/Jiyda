// closed.js — Recently closed: недавно закрытые страницы текущего Space, «Restore all».
// Последнюю закрытую возвращает и Ctrl+Shift+T.
import { S } from './core.js';

(() => {
  const layer = S.h('div', { class: 'layer light', hidden: true });
  const backdrop = S.h('div', { class: 'backdrop' });
  const title = S.h('h2');
  const all = S.h('button', { class: 'link-btn' });
  const list = S.h('div', { class: 'panel-body', attrs: { role: 'listbox' } });
  const panel = S.h('div', { class: 'panel closed-panel', attrs: { role: 'dialog', 'aria-modal': 'true' } },
    S.h('div', { class: 'panel-head' }, title, all), list);
  layer.append(backdrop, panel);
  S.$('#layers').append(layer);
  backdrop.addEventListener('mousedown', () => S.layers.close('closed'));

  let rows = [], sel = 0;
  const restore = (c) => { S.layers.close('closed'); S.post({ type: 'restore_closed', index: c.index }); };

  function draw() {
    title.textContent = S.t('Recently closed');
    all.textContent = S.t('Restore all');
    const closed = (S.state.closed || []).slice(0, 25);
    all.hidden = closed.length < 2;
    rows = closed.map((c, i) => {
      const el = S.h('button', { class: 'row', attrs: { role: 'option', tabindex: '-1' } },
        S.fav(c),
        S.h('span', { class: 'row-main' }, S.h('span', { class: 'row-title', text: S.titleOf(c) }), S.h('span', { class: 'row-sub', text: S.host(c.url) })),
        S.h('span', { class: 'row-meta', text: S.ago(c.closedAt) }));
      el.addEventListener('click', () => restore(c));
      el.addEventListener('mousemove', () => { if (sel !== i) { sel = i; mark(); } });
      return { el, c };
    });
    S.fill(list, ...rows.map((r) => r.el));
    if (!rows.length) list.append(S.h('div', { class: 'empty' }, S.h('b', { text: S.t('Nothing closed yet') }), S.t('Closed pages will show up here for a while.')));
    sel = Math.min(sel, Math.max(0, rows.length - 1));
    mark();
  }
  function mark() { rows.forEach((r, i) => r.el.classList.toggle('sel', i === sel)); }

  all.addEventListener('click', () => { S.layers.close('closed'); S.post({ type: 'restore_all_closed' }); });
  layer.addEventListener('keydown', (e) => {
    if (e.key === 'ArrowDown') sel = Math.min(rows.length - 1, sel + 1);
    else if (e.key === 'ArrowUp') sel = Math.max(0, sel - 1);
    else if (e.key === 'Enter' && rows[sel]) { restore(rows[sel].c); return; }
    else return;
    e.preventDefault();
    mark();
    if (rows[sel]) rows[sel].el.scrollIntoView({ block: 'nearest' });
  });

  S.layers.register('closed', {
    full: true,
    open() {
      layer.hidden = false;
      sel = 0;
      panel.setAttribute('aria-label', S.t('Recently closed'));
      draw();
      requestAnimationFrame(() => { layer.classList.add('open'); panel.tabIndex = -1; panel.focus(); });
    },
    close() {
      S.hide(layer);
    },
  });
  S.on('render', () => { if (S.layers.isOpen('closed')) draw(); });
  S.on('command:closed', () => S.layers.toggle('closed'));
})();
