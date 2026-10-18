// bookmarks.js — Закладки (Ctrl+Shift+O): папки и ссылки, поиск по всем, путь папок сверху.
// На строке: открыть фоном, изменить (название и адрес), переместить, удалить (с Undo).
// Звёздочка в адресной строке и Ctrl+Shift+D добавляют страницу (addressbar.js).
import { S } from './core.js';

(() => {
  const layer = S.h('div', { class: 'layer', hidden: true });
  const backdrop = S.h('div', { class: 'backdrop' });
  const search = S.h('input', { attrs: { spellcheck: 'false', autocomplete: 'off' } });
  const title = S.h('h2');
  const folderBtn = S.h('button', { class: 'btn ghost' });
  const importBtn = S.h('button', { class: 'btn ghost' });
  const closeBtn = S.iconBtn('x', 'Close', () => S.layers.close('bookmarks'), 'sm');
  const crumbs = S.h('nav', { class: 'bm-crumbs' });
  const list = S.h('div', { class: 'panel-body', attrs: { role: 'listbox' } });
  const panel = S.h('div', { class: 'panel history bookmarks', attrs: { role: 'dialog', 'aria-modal': 'true' } },
    S.h('div', { class: 'panel-head' }, title, importBtn, folderBtn, closeBtn),
    S.h('div', { class: 'panel-head', style: { borderBottom: '0', paddingTop: '10px' } }, S.h('label', { class: 'search-field' }, S.icon('search', 'i16'), search)),
    crumbs, list);
  layer.append(backdrop, panel);
  S.$('#layers').append(layer);
  backdrop.addEventListener('mousedown', () => S.layers.close('bookmarks'));

  let items = [], folder = null, rows = [], sel = 0, editing = null;
  S.on('data:bookmarks', (b) => {
    items = (b && b.items) || [];
    // Открытую папку удалили — наверх.
    if (folder !== null && !find(folder)) folder = null;
    if (S.layers.isOpen('bookmarks')) draw();
  });

  function find(id, nodes = items) {
    for (const n of nodes) {
      if (n.id === id) return n;
      const f = n.children && find(id, n.children);
      if (f) return f;
    }
    return null;
  }
  // Цепочка папок до id (без корня).
  function pathTo(id, nodes = items, path = []) {
    for (const n of nodes) {
      if (n.id === id) return [...path, n];
      if (n.children) { const p = pathTo(id, n.children, [...path, n]); if (p) return p; }
    }
    return null;
  }
  const count = (n) => n.children ? n.children.reduce((s, c) => s + count(c), 0) : 1;
  const children = () => (folder === null ? items : (find(folder) || {}).children || []);

  function go(id) {
    folder = id; sel = 0; editing = null;
    search.value = '';
    draw();
    search.focus();
  }

  function drawCrumbs() {
    const path = folder === null ? [] : pathTo(folder) || [];
    const parts = [S.h('button', { text: S.t('Bookmarks'), on: { click: () => go(null) } })];
    for (const n of path) parts.push(S.icon('chevron-right', 'i12'), S.h('button', { text: n.title || S.t('Untitled folder'), on: { click: () => go(n.id) } }));
    crumbs.hidden = !!search.value.trim();
    S.fill(crumbs, ...parts);
  }

  // Строка-редактор: название (и адрес у ссылки). Enter — сохранить, Esc — отменить.
  function editor(n) {
    const name = S.h('input', { type: 'text', value: n.title, attrs: { 'aria-label': S.t('Name'), spellcheck: 'false' } });
    const url = n.children ? null : S.h('input', { type: 'text', value: n.url, attrs: { 'aria-label': S.t('Address'), spellcheck: 'false' } });
    const save = () => {
      editing = null;
      S.post({ type: 'bookmark_edit', id: n.id, title: name.value, url: url ? url.value : null });
    };
    const cancel = () => { editing = null; draw(); search.focus(); };
    for (const inp of [name, url]) {
      if (!inp) continue;
      inp.addEventListener('keydown', (e) => {
        if (e.key === 'Enter') { e.preventDefault(); save(); }
        else if (e.key === 'Escape') { e.preventDefault(); e.stopPropagation(); cancel(); }
        e.stopPropagation();
      });
    }
    const el = S.h('div', { class: 'row bm-edit' },
      S.h('span', { class: 'ico' }, S.icon(n.children ? 'folder' : 'star', 'i16')), name, url,
      S.h('button', { class: 'btn primary', text: S.t('Save'), on: { click: save } }),
      S.iconBtn('x', S.t('Cancel'), cancel, 'sm'));
    requestAnimationFrame(() => { name.focus(); name.select(); });
    return el;
  }

  // Куда переместить: все папки (кроме самой папки и её вложенных) и верхний уровень.
  function moveMenu(anchor, n) {
    const out = [{ label: S.t('Top level'), icon: 'star', disabled: folder === null && !search.value.trim() && items.includes(n), action: () => S.post({ type: 'bookmark_move', id: n.id, parent: null }) }];
    const walk = (nodes, depth) => {
      for (const f of nodes) {
        if (!f.children || f.id === n.id) continue;
        out.push({ label: ' '.repeat(depth) + (f.title || S.t('Untitled folder')), icon: 'folder', disabled: f.children.includes(n),
          action: () => S.post({ type: 'bookmark_move', id: n.id, parent: f.id }) });
        walk(f.children, depth + 1);
      }
    };
    walk(items, 0);
    const here = children();
    const i = here.indexOf(n);
    const parent = folder;
    S.menu.open(anchor, [
      i > 0 ? { label: S.t('Move up'), icon: 'arrow-up', action: () => S.post({ type: 'bookmark_move', id: n.id, parent, index: i - 1 }) } : null,
      i >= 0 && i < here.length - 1 ? { label: S.t('Move down'), icon: 'chevron-down', action: () => S.post({ type: 'bookmark_move', id: n.id, parent, index: i + 2 }) } : null,
      i >= 0 && (i > 0 || i < here.length - 1) ? 'sep' : null,
      { label: S.t('Move to folder'), icon: 'folder', submenu: out },
    ]);
  }

  function row(n, sub) {
    if (editing === n.id) return editor(n);
    const i = rows.length;
    const isFolder = !!n.children;
    const acts = isFolder ? [
      S.iconBtn('external', S.t('Open all'), (e) => { e.stopPropagation(); S.post({ type: 'bookmark_open_all', id: n.id }); }, 'sm'),
      S.iconBtn('pencil', S.t('Rename'), (e) => { e.stopPropagation(); editing = n.id; draw(); }, 'sm'),
      S.iconBtn('move', S.t('Move'), (e) => { e.stopPropagation(); moveMenu(e.currentTarget, n); }, 'sm'),
      S.iconBtn('trash', S.t('Delete'), (e) => { e.stopPropagation(); S.post({ type: 'bookmark_remove', id: n.id }); }, 'sm'),
    ] : [
      S.iconBtn('external', S.t('Open in background'), (e) => { e.stopPropagation(); open(n, true); }, 'sm'),
      S.iconBtn('pencil', S.t('Edit'), (e) => { e.stopPropagation(); editing = n.id; draw(); }, 'sm'),
      S.iconBtn('move', S.t('Move'), (e) => { e.stopPropagation(); moveMenu(e.currentTarget, n); }, 'sm'),
      S.iconBtn('trash', S.t('Delete'), (e) => { e.stopPropagation(); S.post({ type: 'bookmark_remove', id: n.id }); }, 'sm'),
    ];
    const el = S.h('div', { class: 'row' + (isFolder ? ' bm-folder' : ''), attrs: { role: 'option', tabindex: '-1' }, data: { id: n.id } },
      isFolder ? S.h('span', { class: 'ico' }, S.icon('folder', 'i16')) : S.fav({ url: n.url, title: n.title, favicon: icon(n.url) }),
      S.h('span', { class: 'row-main' }, S.h('span', { class: 'row-title', text: n.title || (isFolder ? S.t('Untitled folder') : n.url) }),
        S.h('span', { class: 'row-sub', text: sub || (isFolder ? '' : S.host(n.url)) })),
      isFolder ? S.h('span', { class: 'row-meta hide-on-hover', text: String(count(n)) }) : null,
      S.h('span', { class: 'row-acts' }, ...acts));
    el.addEventListener('click', () => activate(n, false));
    el.addEventListener('auxclick', (e) => { if (e.button === 1 && !isFolder) open(n, true); });
    el.addEventListener('mousemove', () => { if (sel !== i) { sel = i; mark(false); } });
    rows.push({ el, n });
    return el;
  }

  // Значок сайта — из истории (там ядро присылает сохранённые значки).
  let icons = new Map();
  S.on('data:history', (h) => { icons = new Map((h || []).filter((e) => e.favicon).map((e) => [S.host(e.url), e.favicon])); if (S.layers.isOpen('bookmarks') && editing === null) draw(); });
  const icon = (url) => icons.get(S.host(url));

  function draw() {
    title.textContent = S.t('Bookmarks');
    S.fill(folderBtn, S.icon('folder-plus', 'i16'), S.t('New folder'));
    S.fill(importBtn, S.icon('download', 'i16'), S.t('Import…'));
    closeBtn.title = S.t('Close');
    drawCrumbs();
    rows = [];
    const out = [];
    const q = search.value.trim().toLowerCase();
    if (q) {
      const hits = [];
      const walk = (nodes, path) => {
        for (const n of nodes) {
          const name = (n.title || '').toLowerCase();
          if (n.children) {
            if (name.includes(q)) hits.push([n, path]);
            walk(n.children, path ? `${path} › ${n.title}` : n.title);
          } else if (name.includes(q) || n.url.toLowerCase().includes(q)) hits.push([n, path]);
        }
      };
      walk(items, '');
      for (const [n, path] of hits.slice(0, 300)) out.push(row(n, n.children ? path : [S.host(n.url), path].filter(Boolean).join(' · ')));
      if (!hits.length) out.push(S.h('div', { class: 'empty' }, S.h('b', { text: S.t('Nothing found') }), S.t('Try a different word.')));
    } else {
      for (const n of children()) out.push(row(n));
      if (!children().length) {
        out.push(folder === null
          ? S.h('div', { class: 'empty' }, S.h('b', { text: S.t('No bookmarks yet') }),
            S.t('Press ☆ in the address bar or {0} to bookmark a page.', S.keyOf('bookmark') || 'Ctrl Shift D'),
            S.h('div', { style: { marginTop: '14px' } }, S.h('button', { class: 'btn', text: S.t('Import from Chrome or Edge'), on: { click: openImport } })))
          : S.h('div', { class: 'empty' }, S.h('b', { text: S.t('This folder is empty') })));
      }
    }
    S.fill(list, ...out);
    sel = Math.min(sel, Math.max(0, rows.length - 1));
    mark(false);
  }
  function mark(scroll = true) {
    rows.forEach((r, i) => r.el.classList.toggle('sel', i === sel));
    if (scroll && rows[sel]) rows[sel].el.scrollIntoView({ block: 'nearest' });
  }
  function open(n, bg) {
    if (!bg) S.layers.close('bookmarks');
    S.post({ type: 'navigate', text: n.url, mode: bg ? 'background' : 'new' });
  }
  function activate(n, bg) {
    if (n.children) go(n.id);
    else open(n, bg);
  }
  function openImport() {
    S.layers.close('bookmarks');
    S.layers.open('settings', { section: 'import' });
  }

  folderBtn.addEventListener('click', () => S.post({ type: 'bookmark_folder', parent: folder, title: S.t('New folder') }));
  importBtn.addEventListener('click', openImport);
  search.addEventListener('input', () => { sel = 0; editing = null; draw(); });
  layer.addEventListener('keydown', (e) => {
    if (S.menu.isOpen() || editing !== null) return;
    const inSearch = document.activeElement === search;
    if (e.key === 'ArrowDown') sel = Math.min(rows.length - 1, sel + 1);
    else if (e.key === 'ArrowUp') sel = Math.max(0, sel - 1);
    else if (e.key === 'Enter' && rows[sel]) { activate(rows[sel].n, e.altKey || e.ctrlKey); return; }
    else if (e.key === 'Delete' && rows[sel] && !(inSearch && search.value)) S.post({ type: 'bookmark_remove', id: rows[sel].n.id });
    else if (e.key === 'F2' && rows[sel]) { editing = rows[sel].n.id; draw(); e.preventDefault(); return; }
    // Наверх: Backspace в пустом поиске или ←.
    else if ((e.key === 'Backspace' && !search.value) || (e.key === 'ArrowLeft' && !(inSearch && search.value))) {
      if (folder === null) return;
      const path = pathTo(folder) || [];
      go(path.length > 1 ? path[path.length - 2].id : null);
    } else return;
    e.preventDefault();
    mark();
  });

  S.layers.register('bookmarks', {
    full: true,
    onEsc() {
      if (editing !== null) { editing = null; draw(); return false; }
      return true;
    },
    open() {
      layer.hidden = false;
      search.value = '';
      search.placeholder = S.t('Search bookmarks…');
      search.setAttribute('aria-label', S.t('Search bookmarks…'));
      panel.setAttribute('aria-label', S.t('Bookmarks'));
      sel = 0; editing = null;
      S.loadHistory();
      draw();
      requestAnimationFrame(() => { layer.classList.add('open'); search.focus(); });
    },
    close() {
      editing = null;
      S.hide(layer);
    },
  });
  S.on('command:bookmarks', () => S.layers.toggle('bookmarks'));
})();
