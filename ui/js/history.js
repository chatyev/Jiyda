// history.js — История: поиск, фильтр (All / Today / This Week / This Month), строки по дням.
// На строке: Open, Remove (с Undo), Restore context — открыть страницы того же времени.
import { S } from './core.js';

(() => {
  const layer = S.h('div', { class: 'layer', hidden: true });
  const backdrop = S.h('div', { class: 'backdrop' });
  const search = S.h('input', { attrs: { spellcheck: 'false', autocomplete: 'off' } });
  const seg = S.h('div', { class: 'seg', attrs: { role: 'tablist' } });
  const clearBtn = S.h('button', { class: 'btn ghost' });
  const closeBtn = S.iconBtn('x', 'Close', () => S.layers.close('history'), 'sm');
  const title = S.h('h2');
  const list = S.h('div', { class: 'panel-body', attrs: { role: 'listbox' } });
  const panel = S.h('div', { class: 'panel history', attrs: { role: 'dialog', 'aria-modal': 'true' } },
    S.h('div', { class: 'panel-head' }, title, clearBtn, closeBtn),
    S.h('div', { class: 'panel-head', style: { borderBottom: '0', paddingTop: '10px' } }, S.h('label', { class: 'search-field' }, S.icon('search', 'i16'), search), seg),
    list);
  layer.append(backdrop, panel);
  S.$('#layers').append(layer);
  backdrop.addEventListener('mousedown', () => S.layers.close('history'));

  let entries = [], filter = 'all', rows = [], sel = 0, limit = 200;
  S.on('data:history', (h) => { entries = h || []; if (S.layers.isOpen('history')) draw(); });

  function startOf(kind) {
    const d = new Date();
    d.setHours(0, 0, 0, 0);
    if (kind === 'week') d.setDate(d.getDate() - ((d.getDay() + 6) % 7));
    if (kind === 'month') d.setDate(1);
    return d.getTime() / 1000;
  }
  function dayLabel(ts) {
    const d = new Date(ts * 1000);
    const today = new Date(); today.setHours(0, 0, 0, 0);
    const diff = Math.round((today - new Date(d.getFullYear(), d.getMonth(), d.getDate())) / 86400000);
    if (diff === 0) return S.t('Today');
    if (diff === 1) return S.t('Yesterday');
    return d.toLocaleDateString(S.cfg.lang, { weekday: 'long', day: 'numeric', month: 'long' });
  }

  function draw() {
    title.textContent = S.t('History');
    clearBtn.textContent = S.t('Clear history…');
    closeBtn.title = S.t('Close');
    S.fill(seg, ...[['all', 'All'], ['today', 'Today'], ['week', 'This Week'], ['month', 'This Month']].map(([k, l]) =>
      S.h('button', { text: S.t(l), attrs: { role: 'tab', 'aria-selected': filter === k ? 'true' : 'false' }, on: { click: () => { filter = k; sel = 0; limit = 200; draw(); } } })));
    const q = search.value.trim().toLowerCase();
    const from = filter === 'all' ? 0 : startOf(filter);
    const hits = entries.filter((e) => e.ts >= from && (!q || (e.title || '').toLowerCase().includes(q) || e.url.toLowerCase().includes(q)));
    rows = [];
    const out = [];
    let lastDay = '';
    for (const e of hits.slice(0, limit)) {
      const day = dayLabel(e.ts);
      if (day !== lastDay) { out.push(S.h('div', { class: 'group-label', text: day })); lastDay = day; }
      const i = rows.length;
      const el = S.h('div', { class: 'row', attrs: { role: 'option', tabindex: '-1' } },
        S.fav({ url: e.url, title: e.title, favicon: e.favicon }),
        S.h('span', { class: 'row-main' }, S.h('span', { class: 'row-title', text: e.title || e.url }), S.h('span', { class: 'row-sub', text: S.host(e.url) })),
        S.h('span', { class: 'row-meta hide-on-hover', text: S.time(e.ts) }),
        S.h('span', { class: 'row-acts' },
          S.iconBtn('external', S.t('Open'), (ev) => { ev.stopPropagation(); open(e, true); }, 'sm'),
          S.iconBtn('history', S.t('Restore context'), (ev) => { ev.stopPropagation(); S.layers.close('history'); S.post({ type: 'history_context', url: e.url, ts: e.ts }); }, 'sm'),
          S.iconBtn('trash', S.t('Remove'), (ev) => { ev.stopPropagation(); S.post({ type: 'history_remove', url: e.url }); }, 'sm')));
      el.addEventListener('click', () => open(e, false));
      el.addEventListener('auxclick', (ev) => { if (ev.button === 1) open(e, true); });
      el.addEventListener('mousemove', () => { if (sel !== i) { sel = i; mark(false); } });
      rows.push({ el, e });
      out.push(el);
    }
    if (hits.length > limit) {
      out.push(S.h('div', { style: { textAlign: 'center', padding: '12px' } },
        S.h('button', { class: 'btn', text: S.t('Show more'), on: { click: () => { limit += 300; draw(); } } })));
    }
    if (!hits.length) {
      out.push(S.h('div', { class: 'empty' }, S.h('b', { text: q ? S.t('Nothing found') : S.t('No history yet') }),
        q ? S.t('Try a different word or period.') : S.t('Pages you visit will appear here.')));
    }
    S.fill(list, ...out);
    sel = Math.min(sel, Math.max(0, rows.length - 1));
    mark(false);
  }
  function mark(scroll = true) {
    rows.forEach((r, i) => r.el.classList.toggle('sel', i === sel));
    if (scroll && rows[sel]) rows[sel].el.scrollIntoView({ block: 'nearest' });
  }
  function open(e, bg) {
    if (!bg) S.layers.close('history');
    S.post({ type: 'navigate', text: e.url, mode: bg ? 'background' : 'new' });
  }

  search.addEventListener('input', () => { sel = 0; limit = 200; draw(); });
  layer.addEventListener('keydown', (ev) => {
    if (S.menu.isOpen()) return;
    if (ev.key === 'ArrowDown') sel = Math.min(rows.length - 1, sel + 1);
    else if (ev.key === 'ArrowUp') sel = Math.max(0, sel - 1);
    else if (ev.key === 'Enter' && rows[sel]) { open(rows[sel].e, ev.altKey || ev.ctrlKey); return; }
    else if (ev.key === 'Delete' && rows[sel] && document.activeElement !== search) S.post({ type: 'history_remove', url: rows[sel].e.url });
    else return;
    ev.preventDefault();
    mark();
  });
  clearBtn.addEventListener('click', () => { S.layers.close('history'); S.palette('confirm', { what: 'data' }); });

  S.layers.register('history', {
    full: true,
    open() {
      layer.hidden = false;
      search.value = '';
      search.placeholder = S.t('Search history…');
      search.setAttribute('aria-label', S.t('Search history…'));
      panel.setAttribute('aria-label', S.t('History'));
      filter = 'all'; sel = 0; limit = 200;
      S.loadHistory();
      draw();
      requestAnimationFrame(() => { layer.classList.add('open'); search.focus(); });
    },
    close() {
      S.hide(layer);
    },
  });
  S.on('command:history', () => S.layers.toggle('history'));
})();
