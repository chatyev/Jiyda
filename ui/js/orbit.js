// orbit.js — Orbit: все открытые страницы вместо панели вкладок.
//   Now — страницы, с которыми работают; Later — отложенные; Archive — закрытые и архив.
//   Мало страниц — карточки с превью; много (больше 12) — смысловые группы (Development 12 …),
//   внутри группы — карточки. Сверху — Context: цепочка переходов текущей страницы.
//   Поиск — по названию, адресу, сайту, группе и Space (во всех Spaces).
//   Клавиатура: стрелки, Enter — открыть, Delete — закрыть, L — в Later, S — Split,
//   Ctrl+клик / пробел — выбрать несколько. Карточку можно перетащить к краю — Split View.
import { S } from './core.js';

(() => {
  const MANY = 12;
  const GROUP_NAMES = {
    development: 'Development', documentation: 'Documentation', design: 'Design', research: 'Research',
    productivity: 'Productivity', communication: 'Communication', ai: 'AI', shopping: 'Shopping',
    travel: 'Travel', media: 'Media', social: 'Social', news: 'News', finance: 'Finance', search: 'Searches', other: 'Other',
  };
  const groupName = (g) => (g.key === 'site' ? g.label : S.t(GROUP_NAMES[g.key] || g.key));
  const groupOfPage = (p) => {
    if (!p.group) return '';
    if (p.group.startsWith('site:')) return p.group.slice(5);
    return S.t(GROUP_NAMES[p.group] || p.group);
  };
  S.groupName = groupName;

  // ---------- Превью страниц ----------
  // Снимки приходят от ядра в полном размере; храним уменьшенные (для карточек)
  // и последний полный снимок активной страницы (размытый фон Orbit).
  const previews = new Map();
  let backdropShot = null;
  S.on('preview', (id, uri) => {
    const img = new Image();
    img.onload = () => {
      const w = 360, h = Math.round(w * img.height / img.width) || 210;
      const c = document.createElement('canvas');
      c.width = w; c.height = Math.min(h, 420);
      c.getContext('2d').drawImage(img, 0, 0, w, h);
      previews.set(id, c.toDataURL('image/jpeg', 0.82));
      if (id === S.state.active) backdropShot = uri;
      if (S.layers.isOpen('orbit')) { if (id === S.state.active) setBackdrop(); refreshShot(id); }
    };
    img.src = uri;
  });
  S.previewOf = (id) => previews.get(id);

  // ---------- Разметка ----------
  const layer = S.h('div', { class: 'layer', id: 'orbit-layer', hidden: true });
  const backdrop = S.h('div', { class: 'backdrop' });
  const shotImg = S.h('img', { alt: '' });
  backdrop.append(shotImg);
  const search = S.h('input', { attrs: { 'aria-label': 'Search open pages', spellcheck: 'false', autocomplete: 'off', role: 'combobox', 'aria-expanded': 'true', 'aria-controls': 'orbit-list' } });
  const seg = S.h('div', { class: 'seg', attrs: { role: 'tablist' } });
  const spaceChip = S.h('button', { class: 'btn ghost' });
  const closeBtn = S.iconBtn('x', 'Close', () => S.layers.close('orbit'), 'sm');
  // Фильтр по категориям (Разработка, Документация, сайт…): «Все» и группы из state.groups.
  const filterBar = S.h('div', { class: 'orbit-filter', hidden: true, attrs: { role: 'toolbar' } });
  const body = S.h('div', { class: 'panel-body orbit-body', id: 'orbit-list', attrs: { role: 'listbox', 'aria-multiselectable': 'true' } });
  const pickbar = S.h('div', { class: 'pickbar', hidden: true });
  const foot = S.h('div', { class: 'orbit-foot' });
  const panel = S.h('div', { class: 'panel orbit', attrs: { role: 'dialog', 'aria-modal': 'true' } },
    S.h('div', { class: 'panel-head' },
      S.h('label', { class: 'search-field' }, S.icon('search', 'i16'), search),
      seg, spaceChip, closeBtn),
    filterBar, body, pickbar, foot);
  const zoneL = S.h('div', { class: 'drop-zone left' });
  const zoneR = S.h('div', { class: 'drop-zone right' });
  layer.append(backdrop, panel, zoneL, zoneR);
  S.$('#layers').append(layer);
  backdrop.addEventListener('mousedown', () => S.layers.close('orbit'));

  let tab = 'now', expanded = null, focusables = [], sel = 0, picked = new Set(), dragging = null;
  // Выбранная категория: ключ как у страницы в p.group («development», «site:github.com»); null — все.
  let category = null;
  const groupKey = (g) => (g.key === 'site' ? 'site:' + g.label : g.key);
  const inCategory = (p) => !category || p.group === category;

  function drawFilter() {
    const groups = S.state.groups || [];
    if (category && !groups.some((g) => groupKey(g) === category)) category = null;
    // Фильтр нужен, когда есть из чего выбирать: две категории и больше, вкладка Now.
    filterBar.hidden = tab !== 'now' || groups.length < 2;
    if (filterBar.hidden) return;
    filterBar.setAttribute('aria-label', S.t('Categories'));
    const now = S.pagesHere().filter((p) => p.state === 'now');
    const btn = (key, label, count) => S.h('button', {
      class: 'filter-chip',
      attrs: { 'aria-pressed': category === key ? 'true' : 'false' },
      on: { click: () => { category = key; sel = 0; draw(); search.focus(); } },
    }, S.h('span', { text: label }), S.h('span', { class: 'count', text: count }));
    S.fill(filterBar, btn(null, S.t('All'), now.length),
      ...groups.map((g) => btn(groupKey(g), groupName(g), g.ids.filter((id) => now.some((p) => p.id === id)).length)));
  }

  function setBackdrop() {
    if (!backdropShot) { shotImg.classList.remove('on'); return; }
    const ch = S.cfg.chromeH;
    shotImg.style.top = (S.state.fullscreen ? 0 : ch) + 'px';
    shotImg.style.height = `calc(100% - ${S.state.fullscreen ? 0 : ch}px)`;
    shotImg.src = backdropShot;
    shotImg.classList.add('on');
  }

  // ---------- Действия со страницами ----------
  const close = () => S.layers.close('orbit');
  const openPage = (p) => { close(); S.post({ type: 'activate', id: p.id }); };
  function copy(text) {
    const ta = S.h('textarea', { value: text });
    document.body.append(ta); ta.select();
    try { document.execCommand('copy'); } catch (_) { /* нет буфера */ }
    ta.remove();
    S.toast(S.t('Link copied'));
  }
  function pageMenu(p, at) {
    const others = S.state.spaces.map((s, i) => ({ s, i })).filter(({ i }) => i !== S.state.activeSpace);
    const canSplit = S.state.active && S.state.active !== p.id;
    S.menu.open(at, [
      { label: S.t('Open'), icon: 'external', action: () => openPage(p) },
      { label: S.t('Open in Split'), icon: 'columns', disabled: !canSplit, action: () => { close(); S.post({ type: 'split', id: p.id }); } },
      { label: S.t('Move to Space'), icon: 'layers', disabled: !others.length, submenu: others.map(({ s, i }) => ({ label: s.name, action: () => S.post({ type: 'move_to_space', id: p.id, space: i }) })) },
      p.state === 'later'
        ? { label: S.t('Move to Now'), icon: 'arrow-up', action: () => S.post({ type: 'later_undo', id: p.id, activate: false }) }
        : { label: S.t('Move to Later'), icon: 'inbox', kbd: 'L', disabled: p.pinned, action: () => S.post({ type: 'later', id: p.id }) },
      { label: S.t('Duplicate'), icon: 'duplicate', action: () => { close(); S.post({ type: 'duplicate', id: p.id }); } },
      { label: S.t('Copy link'), icon: 'link', action: () => copy(p.url) },
      { label: p.pinned ? S.t('Unpin') : S.t('Pin'), icon: 'pin', action: () => S.post({ type: 'pin', id: p.id, on: !p.pinned }) },
      'sep',
      { label: S.t('Close'), icon: 'x', kbd: 'Del', action: () => S.post({ type: 'close', id: p.id }) },
    ]);
  }

  // ---------- Карточка страницы ----------
  function card(p) {
    const shot = S.h('div', { class: 'shot' });
    const pv = previews.get(p.id);
    if (pv) shot.append(S.h('img', { class: 'preview', src: pv, alt: '', draggable: false }));
    else shot.append(S.fav(p, 'lg'));
    shot.dataset.id = p.id;
    const canSplit = S.state.active && S.state.active !== p.id;
    const acts = S.h('div', { class: 'acts' },
      p.state !== 'later' && !p.pinned ? S.iconBtn('inbox', S.t('Move to Later'), (e) => { e.stopPropagation(); S.post({ type: 'later', id: p.id }); }) : null,
      canSplit ? S.iconBtn('columns', S.t('Open in Split'), (e) => { e.stopPropagation(); close(); S.post({ type: 'split', id: p.id }); }) : null,
      S.iconBtn('more', S.t('More'), (e) => { e.stopPropagation(); pageMenu(p, e.currentTarget); }),
      S.iconBtn('x', S.t('Close'), (e) => { e.stopPropagation(); S.post({ type: 'close', id: p.id }); }));
    const el = S.h('div', {
      class: 'card' + (p.id === S.state.active ? ' active' : '') + (picked.has(p.id) ? ' picked' : ''),
      draggable: true,
      attrs: { role: 'option', tabindex: '-1', 'aria-selected': picked.has(p.id) ? 'true' : 'false', 'aria-label': `${S.titleOf(p)} — ${p.domain}` },
    },
    shot, acts,
    S.h('div', { class: 'meta' }, S.fav(p),
      S.h('div', { class: 'meta-text' },
        S.h('div', { class: 'title', text: S.titleOf(p) }),
        S.h('div', { class: 'domain', text: (p.pinned ? '📌 ' : '') + (p.domain || p.url) }))));
    hookPage(el, p);
    return el;
  }

  function chip(p) {
    const el = S.h('button', { class: 'page-chip' + (p.id === S.state.active ? ' active' : ''), attrs: { role: 'option', tabindex: '-1', title: `${S.titleOf(p)}\n${p.url}` } },
      S.fav(p), S.h('span', { class: 'ellipsis', text: S.titleOf(p) }));
    el.draggable = true;
    hookPage(el, p);
    return el;
  }

  function hookPage(el, p) {
    el.addEventListener('click', (e) => {
      if (e.ctrlKey || e.shiftKey) { togglePick(p.id); return; }
      openPage(p);
    });
    el.addEventListener('auxclick', (e) => { if (e.button === 1) S.post({ type: 'close', id: p.id }); });
    el.addEventListener('mousedown', (e) => { if (e.button === 1) e.preventDefault(); });
    el.addEventListener('contextmenu', (e) => { e.preventDefault(); pageMenu(p, e); });
    el.addEventListener('dragstart', (e) => {
      dragging = p.id;
      e.dataTransfer.setData('text/plain', p.url);
      e.dataTransfer.effectAllowed = 'move';
      if (S.state.active && S.state.active !== p.id) layer.classList.add('dragging-page');
      el.classList.add('dragging');
    });
    el.addEventListener('dragend', () => { dragging = null; layer.classList.remove('dragging-page'); el.classList.remove('dragging'); zoneL.classList.remove('over'); zoneR.classList.remove('over'); });
    focusables.push({ el, page: p, open: () => openPage(p) });
  }

  for (const [zone, side] of [[zoneL, 'left'], [zoneR, 'right']]) {
    zone.addEventListener('dragover', (e) => { if (dragging) { e.preventDefault(); zone.classList.add('over'); } });
    zone.addEventListener('dragleave', () => zone.classList.remove('over'));
    zone.addEventListener('drop', (e) => {
      e.preventDefault();
      const id = dragging;
      dragging = null;
      layer.classList.remove('dragging-page');
      if (id) { close(); S.post({ type: 'split', id, side }); }
    });
  }

  function togglePick(id) {
    if (picked.has(id)) picked.delete(id); else picked.add(id);
    draw();
  }

  // ---------- Строки списков (Later, Archive, поиск) ----------
  function row(p, meta, acts, onOpen) {
    const el = S.h('div', { class: 'row', attrs: { role: 'option', tabindex: '-1' } },
      S.fav(p),
      S.h('span', { class: 'row-main' }, S.h('span', { class: 'row-title', text: S.titleOf(p) }), S.h('span', { class: 'row-sub', text: S.host(p.url) })),
      meta ? S.h('span', { class: 'row-meta hide-on-hover', text: meta }) : null,
      S.h('span', { class: 'row-acts' }, ...acts));
    el.addEventListener('click', onOpen);
    focusables.push({ el, page: p.id ? p : null, open: onOpen });
    return el;
  }

  // ---------- Отрисовка ----------
  function section(title, count, ...kids) {
    return [S.h('div', { class: 'orbit-title' }, S.h('h3', { text: title }), count != null ? S.h('span', { class: 'muted', text: count }) : null), ...kids];
  }

  function contextStrip(nowPages) {
    const act = S.activePage();
    if (!act) return [];
    const related = new Map();
    const byId = (id) => nowPages.find((p) => p.id === id) || S.page(id);
    let cur = act;
    while (cur && cur.parentPageId) { const par = byId(cur.parentPageId); if (!par || related.has(par.id)) break; related.set(par.id, par); cur = par; }
    for (const p of S.pagesHere()) {
      if (p.id === act.id) continue;
      if (p.parentPageId === act.id || (act.parentPageId && p.parentPageId === act.parentPageId)) related.set(p.id, p);
    }
    if (!related.size) return [];
    const chips = [...related.values()].slice(0, 8).map((p) => chip(p));
    return section(S.t('Context'), S.t('linked to {0}', act.domain || S.titleOf(act)), S.h('div', { class: 'cluster-chips' }, ...chips));
  }

  function drawNow() {
    const now = S.pagesHere().filter((p) => p.state === 'now' && inCategory(p));
    // Выбрана категория — только её страницы, карточками, без контекста и групп.
    if (category) {
      const g = (S.state.groups || []).find((x) => groupKey(x) === category);
      return section(g ? groupName(g) : S.t('Now'), now.length, S.h('div', { class: 'cards' }, ...now.map(card)));
    }
    const out = [...contextStrip(now)];
    const newCard = S.h('button', { class: 'card new', attrs: { tabindex: '-1' }, on: { click: () => { close(); S.cmd('new_tab'); } } },
      S.h('div', { class: 'shot' }, S.icon('plus', 'i20')),
      S.h('div', { class: 'meta' }, S.h('div', { class: 'meta-text' }, S.h('div', { class: 'title', text: S.t('New page') }), S.h('div', { class: 'domain', text: S.keyOf('new_tab') }))));
    focusables.push({ el: newCard, open: () => { close(); S.cmd('new_tab'); } });
    if (!now.length) {
      out.push(S.h('div', { class: 'empty' }, S.h('b', { text: S.t('No open pages in this Space') }), S.t('Start with a new page or bring something back from Later.')));
      out.push(S.h('div', { class: 'cards' }, newCard));
      return out;
    }
    if (now.length <= MANY) {
      out.push(...section(S.t('Now'), now.length, S.h('div', { class: 'cards' }, ...now.map(card), newCard)));
      return out;
    }
    // Много страниц: сначала смысловые группы.
    out.push(...section(S.t('Now'), S.t('{0} pages in {1} groups', now.length, S.state.groups.length)));
    for (const g of S.state.groups) {
      const pages = g.ids.map((id) => now.find((p) => p.id === id)).filter(Boolean);
      if (!pages.length) continue;
      const key = g.key + ':' + g.label;
      const open = expanded === key;
      const more = S.iconBtn('more', S.t('Group options'), (e) => { e.stopPropagation(); groupMenu(g, pages, e.currentTarget); }, 'sm');
      const head = S.h('div', { class: 'cluster-head' },
        S.h('span', { class: 'name', text: groupName(g) }), S.h('span', { class: 'count', text: pages.length }),
        S.h('span', { class: 'spacer' }),
        S.h('button', { class: 'link-btn', text: open ? S.t('Collapse') : S.t('Show all'), on: { click: () => { expanded = open ? null : key; draw(); } } }),
        more);
      const cl = S.h('section', { class: 'cluster', attrs: { 'aria-label': groupName(g) } }, head);
      if (open) cl.append(S.h('div', { class: 'cards' }, ...pages.map(card)));
      else {
        const shown = pages.slice(0, 5);
        const chips = S.h('div', { class: 'cluster-chips' }, ...shown.map(chip));
        if (pages.length > shown.length) {
          const m = S.h('button', { class: 'page-chip more', text: '+' + (pages.length - shown.length), attrs: { tabindex: '-1', 'aria-label': S.t('Show all') }, on: { click: () => { expanded = key; draw(); } } });
          focusables.push({ el: m, open: () => { expanded = key; draw(); } });
          chips.append(m);
        }
        cl.append(chips);
      }
      out.push(cl);
    }
    out.push(S.h('div', { class: 'cards', style: { marginTop: '16px' } }, newCard));
    return out;
  }

  function groupMenu(g, pages, at) {
    const ids = pages.map((p) => p.id);
    S.menu.open(at, [
      { label: S.t('Ask Jiyda about this group'), icon: 'sparkle', action: () => { close(); S.layers.open('ask', { pages: ids, action: 'research' }); } },
      { label: S.t('Move group to Later'), icon: 'inbox', action: () => ids.forEach((id) => S.post({ type: 'later', id })) },
      'sep',
      { label: S.t('Close group'), icon: 'x', danger: true, action: () => ids.forEach((id) => S.post({ type: 'close', id })) },
    ]);
  }

  function drawLater() {
    const later = S.pagesHere().filter((p) => p.state === 'later');
    if (!later.length) return [S.h('div', { class: 'empty' }, S.h('b', { text: S.t('Nothing saved for later') }), S.t('Move pages here to clear your head without losing them.'))];
    later.sort((a, b) => b.lastActiveAt - a.lastActiveAt);
    return section(S.t('Later'), later.length, ...later.map((p) => row(p, S.ago(p.lastActiveAt), [
      S.iconBtn('arrow-up', S.t('Move to Now'), (e) => { e.stopPropagation(); S.post({ type: 'later_undo', id: p.id, activate: false }); }, 'sm'),
      S.iconBtn('x', S.t('Close'), (e) => { e.stopPropagation(); S.post({ type: 'close', id: p.id }); }, 'sm'),
    ], () => openPage(p))));
  }

  function drawArchive() {
    const closed = S.state.closed || [];
    if (!closed.length) return [S.h('div', { class: 'empty' }, S.h('b', { text: S.t('Archive is empty') }), S.t('Closed pages and pages you haven’t opened in a while end up here.'))];
    const clear = S.h('button', { class: 'link-btn', text: S.t('Clear archive'), on: { click: () => S.post({ type: 'clear_closed' }) } });
    return [
      S.h('div', { class: 'orbit-title' }, S.h('h3', { text: S.t('Archive') }), S.h('span', { class: 'muted', text: closed.length }), S.h('span', { style: { flex: 1 } }), clear),
      ...closed.map((c) => row(c, S.ago(c.closedAt), [
        S.iconBtn('undo', S.t('Restore'), (e) => { e.stopPropagation(); close(); S.post({ type: 'restore_closed', index: c.index }); }, 'sm'),
      ], () => { close(); S.post({ type: 'restore_closed', index: c.index }); })),
    ];
  }

  function drawSearch(q) {
    const ql = q.toLowerCase();
    const has = (s) => (s || '').toLowerCase().includes(ql);
    const spaceName = (id) => (S.state.spaces.find((s) => s.id === id) || {}).name || '';
    const hits = S.state.pages.filter((p) => inCategory(p) && (has(p.title) || has(p.url) || has(p.domain) || has(groupOfPage(p)) || has(spaceName(p.spaceId))));
    const arch = (S.state.closed || []).filter((c) => has(c.title) || has(c.url));
    if (!hits.length && !arch.length) return [S.h('div', { class: 'empty' }, S.h('b', { text: S.t('No pages match “{0}”', q) }), S.t('Press Enter to search the web instead.'))];
    const meta = (p) => [p.state === 'later' ? S.t('Later') : '', p.spaceId !== S.space().id ? spaceName(p.spaceId) : '', groupOfPage(p)].filter(Boolean).join(' · ');
    return [
      ...section(S.t('Pages'), hits.length, ...hits.slice(0, 60).map((p) => row(p, meta(p), [
        S.iconBtn('x', S.t('Close'), (e) => { e.stopPropagation(); S.post({ type: 'close', id: p.id }); }, 'sm'),
      ], () => openPage(p)))),
      ...(arch.length ? section(S.t('Archive'), arch.length, ...arch.slice(0, 20).map((c) => row(c, S.ago(c.closedAt), [], () => { close(); S.post({ type: 'restore_closed', index: c.index }); }))) : []),
    ];
  }

  function draw() {
    const q = search.value.trim();
    focusables = [];
    const here = S.pagesHere();
    const counts = { now: here.filter((p) => p.state === 'now').length, later: here.filter((p) => p.state === 'later').length, archive: (S.state.closed || []).length };
    S.fill(seg, ...[['now', 'Now'], ['later', 'Later'], ['archive', 'Archive']].map(([k, l]) => S.h('button', {
      attrs: { role: 'tab', 'aria-selected': !q && tab === k ? 'true' : 'false' },
      on: { click: () => { tab = k; search.value = ''; sel = 0; draw(); search.focus(); } },
    }, S.t(l), S.h('span', { class: 'count', text: counts[k] }))));
    S.fill(spaceChip, S.h('span', { class: 'space-dot' }), S.space().name, S.icon('chevron-down', 'i12'));
    spaceChip.title = S.t('Switch Space');
    drawFilter();
    const content = q ? drawSearch(q) : tab === 'later' ? drawLater() : tab === 'archive' ? drawArchive() : drawNow();
    S.fill(body, ...content);
    sel = Math.min(sel, Math.max(0, focusables.length - 1));
    mark(false);
    // Выбрано несколько страниц — действия над ними.
    const ids = [...picked].filter((id) => S.page(id));
    picked = new Set(ids);
    pickbar.hidden = !ids.length;
    if (ids.length) {
      S.fill(pickbar, 
        S.h('span', { text: S.t('{0} selected', ids.length) }), S.h('span', { class: 'spacer' }),
        S.h('button', { class: 'btn', on: { click: () => { close(); S.layers.open('ask', { pages: ids, action: ids.length > 1 ? 'compare' : 'summarize' }); } } }, S.icon('sparkle', 'i16'), S.t('Ask Jiyda')),
        S.h('button', { class: 'btn', text: S.t('Move to Later'), on: { click: () => { ids.forEach((id) => S.post({ type: 'later', id })); picked.clear(); } } }),
        S.h('button', { class: 'btn', text: S.t('Close'), on: { click: () => { ids.forEach((id) => S.post({ type: 'close', id })); picked.clear(); } } }),
        S.h('button', { class: 'btn ghost', text: S.t('Clear'), on: { click: () => { picked.clear(); draw(); } } }));
    }
    S.fill(foot, 
      S.h('span', {}, S.h('kbd', { text: '↑↓←→' }), S.t('Navigate')),
      S.h('span', {}, S.h('kbd', { text: 'Enter' }), S.t('Open')),
      S.h('span', {}, S.h('kbd', { text: 'Del' }), S.t('Close')),
      S.h('span', {}, S.h('kbd', { text: 'L' }), S.t('Later')),
      S.h('span', {}, S.h('kbd', { text: 'Ctrl+Click' }), S.t('Select')),
      S.h('span', { class: 'spacer' }),
      S.h('span', {}, S.h('kbd', { text: S.keyOf('orbit') || 'Ctrl Space' }), S.t('Orbit')));
  }

  function refreshShot(id) {
    const shot = body.querySelector(`.shot[data-id="${id}"]`);
    const pv = previews.get(id);
    if (shot && pv) S.fill(shot, S.h('img', { class: 'preview', src: pv, alt: '', draggable: false }));
  }

  function mark(scroll = true) {
    focusables.forEach((f, i) => f.el.classList.toggle('sel', i === sel));
    const f = focusables[sel];
    S.activeOption(search, f && f.el);
    if (f && scroll) f.el.scrollIntoView({ block: 'nearest' });
  }

  // Сколько карточек в строке сетки (для ↑↓).
  function cols(el) {
    const grid = el.closest('.cards');
    if (!grid) return 1;
    const items = [...grid.children];
    const top = items[0].offsetTop;
    const n = items.filter((x) => x.offsetTop === top).length;
    return Math.max(1, n);
  }

  search.addEventListener('input', () => { sel = 0; draw(); });
  layer.addEventListener('keydown', (e) => {
    if (S.menu.isOpen()) return;
    const f = focusables[sel];
    const inSearch = document.activeElement === search;
    const typing = inSearch && search.value && (e.key === 'ArrowLeft' || e.key === 'ArrowRight');
    if (typing) return;
    const n = focusables.length;
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      const step = f ? cols(f.el) : 1;
      sel = Math.max(0, Math.min(n - 1, sel + (e.key === 'ArrowDown' ? step : -step)));
    } else if (e.key === 'ArrowRight') sel = Math.min(n - 1, sel + 1);
    else if (e.key === 'ArrowLeft') sel = Math.max(0, sel - 1);
    else if (e.key === 'Enter') { if (f) f.open(); else if (search.value.trim()) { close(); S.post({ type: 'navigate', text: search.value, mode: 'new' }); } }
    else if (e.key === 'Tab' && !e.ctrlKey) {
      const order = ['now', 'later', 'archive'];
      tab = order[(order.indexOf(tab) + (e.shiftKey ? 2 : 1)) % 3];
      search.value = ''; sel = 0; draw();
    }
    else if ((e.key === 'Delete' || (e.key === 'Backspace' && !search.value)) && f && f.page) { S.post({ type: 'close', id: f.page.id }); }
    else if (!search.value && (e.key === 'l' || e.key === 'L' || e.key === 'д' || e.key === 'Д') && f && f.page && f.page.state === 'now') { S.post({ type: 'later', id: f.page.id }); }
    else if (!search.value && (e.key === ' ') && f && f.page) { togglePick(f.page.id); }
    else if (e.key === 'ContextMenu' && f && f.page) { pageMenu(f.page, f.el); }
    else return;
    e.preventDefault();
    mark();
  });

  S.layers.register('orbit', {
    full: true,
    open(arg = {}) {
      tab = arg.tab || 'now';
      search.value = '';
      search.placeholder = S.t('Search open pages…');
      search.setAttribute('aria-label', S.t('Search open pages…'));
      panel.setAttribute('aria-label', 'Orbit');
      closeBtn.title = S.t('Close'); closeBtn.setAttribute('aria-label', S.t('Close'));
      expanded = null;
      category = null;
      picked.clear();
      // Текущая страница — выбранной.
      sel = 0;
      layer.hidden = false;
      draw();
      const i = focusables.findIndex((f) => f.page && f.page.id === S.state.active);
      if (i >= 0) { sel = i; mark(); }
      shotImg.classList.remove('on');
      if (backdropShot) setBackdrop();
      S.post({ type: 'get', what: 'previews' });
      requestAnimationFrame(() => { layer.classList.add('open'); search.focus(); });
    },
    close() {
      S.hide(layer);
    },
  });
  spaceChip.addEventListener('click', () => {
    const r = spaceChip.getBoundingClientRect();
    S.layers.close('orbit');
    S.layers.open('spaces', { anchor: r });
  });
  S.on('render', () => { if (S.layers.isOpen('orbit')) draw(); });
  S.on('command:orbit', () => S.layers.toggle('orbit'));
})();
