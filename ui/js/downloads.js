// downloads.js — загрузки. Идущая загрузка — маленькая карточка справа внизу
// (имя, «12 MB · 40%», полоска хода); после окончания — «Downloaded»: Open, Show in folder.
// Большой список (Ctrl+J) открывается только по запросу.
import { S } from './core.js';

(() => {
  let items = [];
  const chip = S.h('div', { class: 'dl-chip', hidden: true, attrs: { role: 'status', 'data-hit': '22' } });
  document.body.append(chip);
  const pop = S.h('div', { class: 'pop downloads right', hidden: true, attrs: { role: 'dialog', 'data-hit': '22' } });
  S.$('#popovers').append(pop);
  const search = S.h('input', { attrs: { spellcheck: 'false', autocomplete: 'off' } });
  const list = S.h('div', { class: 'panel-body' });
  let chipId = null, hideTimer = 0, seenDone = new Set();

  const act = (d, action) => S.post({ type: 'download', id: d.id, action });
  function progressText(d) {
    if (d.state === 'running') {
      if (d.total) return `${S.size(d.received)} / ${S.size(d.total)} · ${Math.floor((d.received / d.total) * 100)}%`;
      return d.received ? S.size(d.received) : S.t('Starting…');
    }
    if (d.state === 'done') return [S.size(d.total || d.received), S.ago(d.ts)].filter(Boolean).join(' · ');
    return S.t('Failed');
  }
  function bar(d) {
    const b = S.h('div', { class: 'dl-bar' + (d.state === 'running' && !d.total ? ' indeterminate' : '') }, S.h('i'));
    if (d.total) b.firstChild.style.width = Math.min(100, (d.received / d.total) * 100) + '%';
    return b;
  }

  // ---------- Карточка внизу справа ----------
  function drawChip() {
    const running = items.filter((d) => d.state === 'running');
    let d = running[0] || items.find((x) => x.id === chipId);
    if (!d) { if (!chip.hidden) S.hide(chip); return; }
    chipId = d.id;
    const more = running.length > 1 ? S.h('span', { class: 'chip', text: '+' + (running.length - 1) }) : null;
    const top = S.h('div', { class: 'dl-top' },
      S.icon(d.state === 'done' ? 'check' : d.state === 'failed' ? 'alert' : 'download', 'i16'),
      S.h('div', { class: 'ellipsis', style: { flex: '1' } },
        S.h('div', { class: 'dl-name', text: d.name, title: d.name }),
        S.h('div', { class: 'dl-sub', text: d.state === 'done' ? S.t('Downloaded') : progressText(d) })),
      more,
      d.state === 'running'
        ? S.iconBtn('x', S.t('Cancel download'), () => act(d, 'cancel'), 'sm')
        : S.iconBtn('x', S.t('Dismiss'), () => { chipId = null; S.hide(chip); }, 'sm'));
    const parts = [top];
    if (d.state === 'running') parts.push(bar(d));
    if (d.state === 'done') {
      parts.push(S.h('div', { class: 'dl-acts' },
        S.h('button', { class: 'btn', text: S.t('Open'), on: { click: () => act(d, 'open') } }),
        S.h('button', { class: 'btn ghost', text: S.t('Show in folder'), on: { click: () => act(d, 'reveal') } })));
    }
    S.fill(chip, ...parts);
    chip.setAttribute('aria-label', `${d.name}: ${progressText(d)}`);
    if (chip.hidden) S.show(chip);
    clearTimeout(hideTimer);
    if (d.state !== 'running') hideTimer = setTimeout(() => { if (!chip.matches(':hover')) { chipId = null; S.hide(chip); } }, 8000);
  }
  chip.addEventListener('click', (e) => { if (!e.target.closest('button')) S.cmd('downloads'); });

  // ---------- Список загрузок ----------
  function drawList() {
    const q = search.value.trim().toLowerCase();
    const rows = items.filter((d) => !q || d.name.toLowerCase().includes(q) || d.url.toLowerCase().includes(q)).map((d) => {
      const acts = d.state === 'running'
        ? [S.iconBtn('x', S.t('Cancel download'), (e) => { e.stopPropagation(); act(d, 'cancel'); }, 'sm')]
        : [
          d.state === 'done' ? S.iconBtn('folder', S.t('Show in folder'), (e) => { e.stopPropagation(); act(d, 'reveal'); }, 'sm') : null,
          d.state === 'failed' ? S.iconBtn('rotate-cw', S.t('Try again'), (e) => { e.stopPropagation(); S.post({ type: 'navigate', text: d.url, mode: 'background' }); }, 'sm') : null,
          S.iconBtn('trash', S.t('Remove from list'), (e) => { e.stopPropagation(); act(d, 'remove'); }, 'sm'),
        ];
      const el = S.h('div', { class: 'row', attrs: { tabindex: '-1', role: 'listitem' } },
        S.h('span', { class: 'ico' }, S.icon(d.state === 'failed' ? 'alert' : 'file', 'i16')),
        S.h('div', { class: 'row-main', style: { flexDirection: 'column', alignItems: 'stretch', gap: '4px' } },
          S.h('span', { class: 'row-title', text: d.name, title: d.path || d.url }),
          S.h('span', { class: 'row-sub', style: { maxWidth: 'none' }, text: progressText(d) }),
          d.state === 'running' ? bar(d) : null),
        S.h('span', { class: 'row-acts', style: d.state === 'running' ? { display: 'flex' } : null }, ...acts));
      if (d.state === 'done') el.addEventListener('click', () => act(d, 'open'));
      return el;
    });
    S.fill(list, ...rows);
    if (!rows.length) list.append(S.h('div', { class: 'empty' }, S.h('b', { text: q ? S.t('Nothing found') : S.t('No downloads yet') }), q ? '' : S.t('Files you download will appear here.')));
  }

  S.on('data:downloads', (d) => {
    const before = new Map(items.map((x) => [x.id, x.state]));
    items = d || [];
    // Новая загрузка или только что закончившаяся — показываем карточку.
    for (const x of items) {
      const was = before.get(x.id);
      if ((x.state === 'running' && was === undefined) || (x.state !== 'running' && was === 'running' && !seenDone.has(x.id))) {
        chipId = x.id;
        if (x.state !== 'running') seenDone.add(x.id);
      }
    }
    if (chipId !== null || items.some((x) => x.state === 'running')) drawChip();
    if (S.layers.isOpen('downloads')) drawList();
  });

  S.layers.register('downloads', {
    pop: true,
    open() {
      search.value = '';
      search.placeholder = S.t('Search downloads…');
      search.setAttribute('aria-label', S.t('Search downloads…'));
      pop.setAttribute('aria-label', S.t('Downloads'));
      S.fill(pop, 
        S.h('div', { class: 'panel-head' }, S.h('h2', { text: S.t('Downloads') }),
          S.h('button', { class: 'link-btn', text: S.t('Open folder'), on: { click: () => S.cmd('open_downloads_folder') } }),
          S.iconBtn('x', S.t('Close'), () => S.layers.close('downloads'), 'sm')),
        S.h('div', { style: { padding: '0 12px 8px' } }, S.h('label', { class: 'search-field' }, S.icon('search', 'i16'), search)),
        list);
      drawList();
      S.placeAt(pop, S.$('#menu-btn'), { align: 'right', gap: 8 });
      S.show(pop);
      requestAnimationFrame(() => search.focus());
    },
    close() { S.hide(pop); },
  });
  search.addEventListener('input', drawList);
  S.on('command:downloads', () => S.layers.toggle('downloads'));
})();
