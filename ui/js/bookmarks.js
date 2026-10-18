// bookmarks.js — закладки. Две части:
// • Панель (кнопка 🔖 на верхней панели, Ctrl+Shift+O): плавает справа под панелью поверх
//   страницы, не сдвигая её. Дерево папок, поиск, «Добавить текущую страницу», меню строки
//   (открыть, в новой странице, изменить, переместить, удалить с Undo). Щелчок мимо — закрыть.
// • Окно закладки (Ctrl+D, ☆ в адресной строке): название и папка. Новая закладка
//   сохраняется только по «Сохранить»; уже сохранённую можно изменить или удалить.
//   Второй закладки на тот же адрес не бывает (ядро: Bookmarks::save_page).
// Цвет кнопки, выделения и фокуса — от цвета Space (styles.css, --glow).
import { S } from './core.js';

(() => {
  // ---------- Данные ----------
  let items = [];
  const listeners = [];
  S.on('data:bookmarks', (b) => {
    items = (b && b.items) || [];
    for (const fn of listeners) fn();
  });

  function find(id, nodes = items) {
    for (const n of nodes) {
      if (n.id === id) return n;
      const f = n.children && find(id, n.children);
      if (f) return f;
    }
    return null;
  }
  // Цепочка папок до id (без корня), включая сам узел.
  function pathTo(id, nodes = items, path = []) {
    for (const n of nodes) {
      if (n.id === id) return [...path, n];
      if (n.children) { const p = pathTo(id, n.children, [...path, n]); if (p) return p; }
    }
    return null;
  }
  function byUrl(url, nodes = items) {
    for (const n of nodes) {
      if (!n.children && n.url === url) return n;
      const f = n.children && byUrl(url, n.children);
      if (f) return f;
    }
    return null;
  }
  const parentOf = (id) => { const p = pathTo(id) || []; return p.length > 1 ? p[p.length - 2].id : null; };
  const count = (n) => n.children ? n.children.reduce((s, c) => s + count(c), 0) : 1;
  const folderName = (n) => n.title || S.t('Untitled folder');
  // Все папки по порядку с глубиной — для выбора папки.
  function folders(skip = null) {
    const out = [];
    const walk = (nodes, depth) => {
      for (const f of nodes) {
        if (!f.children || f.id === skip) continue;
        out.push({ f, depth });
        walk(f.children, depth + 1);
      }
    };
    walk(items, 0);
    return out;
  }

  // Значок сайта — из истории (там ядро присылает сохранённые значки).
  let icons = new Map();
  S.on('data:history', (h) => { icons = new Map((h || []).filter((e) => e.favicon).map((e) => [S.host(e.url), e.favicon])); });
  const icon = (url) => icons.get(S.host(url));

  // Страница на экране, которую можно сохранить (только сайт).
  const current = () => { const p = S.activePage(); return p && /^https?:/i.test(p.url) ? p : null; };

  // ---------- Панель ----------
  const btn = S.$('#bm-btn');
  const title = S.h('h2');
  const folderBtn = S.iconBtn('folder-plus', 'New folder', () => S.post({ type: 'bookmark_folder', title: S.t('New folder') }), 'sm');
  const importBtn = S.iconBtn('download', 'Import…', () => openImport(), 'sm');
  const closeBtn = S.iconBtn('x', 'Close', () => S.layers.close('bookmarks'), 'sm');
  const search = S.h('input', { attrs: { spellcheck: 'false', autocomplete: 'off' } });
  const addRow = S.h('button', { class: 'row bm-add' });
  const list = S.h('div', { class: 'panel-body bm-list', attrs: { role: 'tree' } });
  const panel = S.h('div', { class: 'pop bm-panel right', hidden: true, attrs: { role: 'dialog', 'data-hit': '22' } },
    S.h('div', { class: 'bm-head' }, title, folderBtn, importBtn, closeBtn),
    S.h('div', { class: 'bm-search' }, S.h('label', { class: 'search-field' }, S.icon('search', 'i16'), search)),
    addRow, list);
  panel.tabIndex = -1;
  S.$('#popovers').append(panel);

  const expanded = new Set();
  let rows = [], sel = 0, editing = null, returnTo = null;

  function drawButton() {
    const open = S.layers.isOpen('bookmarks');
    btn.classList.toggle('on', open);
    btn.setAttribute('aria-expanded', open ? 'true' : 'false');
    btn.title = S.withKey(S.t('Bookmarks'), 'bookmarks');
    btn.setAttribute('aria-label', open ? S.t('Close bookmarks') : S.t('Open bookmarks'));
  }

  function drawAdd() {
    const p = current();
    addRow.hidden = !p || !!search.value.trim();
    if (addRow.hidden) return;
    const saved = byUrl(p.url);
    S.fill(addRow, S.h('span', { class: 'ico' }, S.icon('star', 'i16')),
      S.h('span', { class: 'row-main' }, S.h('span', { class: 'row-title', text: saved ? S.t('Edit bookmark for this page') : S.t('Add current page') }),
        S.h('span', { class: 'row-sub', text: S.host(p.url) })),
      S.h('kbd', { text: S.keyOf('bookmark') }));
    addRow.onclick = () => {
      if (saved) { reveal(saved.id); startEdit(saved.id); }
      else S.post({ type: 'bookmark_add', url: p.url, title: S.titleOf(p), parent: null });
    };
  }

  // Раскрыть папки до узла и выделить его.
  function reveal(id) {
    for (const f of (pathTo(id) || []).slice(0, -1)) expanded.add(f.id);
    draw();
    const i = rows.findIndex((r) => r.n.id === id);
    if (i >= 0) { sel = i; mark(); }
  }
  function startEdit(id) { editing = id; draw(); }

  // Строка-редактор: название (и адрес у ссылки). Enter — сохранить, Esc — отменить.
  function editor(n, depth) {
    const name = S.h('input', { type: 'text', value: n.title, attrs: { 'aria-label': S.t('Name'), spellcheck: 'false' } });
    const url = n.children ? null : S.h('input', { type: 'text', value: n.url, attrs: { 'aria-label': S.t('Address'), spellcheck: 'false' } });
    const save = () => {
      editing = null;
      S.post({ type: 'bookmark_edit', id: n.id, title: name.value, url: url ? url.value : null });
      requestAnimationFrame(() => search.focus());
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
    const el = S.h('div', { class: 'bm-edit', style: { marginLeft: depth * 16 + 'px' } }, name, url,
      S.h('div', { class: 'bm-edit-acts' },
        S.h('button', { class: 'btn ghost', text: S.t('Cancel'), on: { click: cancel } }),
        S.h('button', { class: 'btn primary', text: S.t('Save'), on: { click: save } })));
    requestAnimationFrame(() => { name.focus(); name.select(); });
    return el;
  }

  // Меню строки (правая кнопка, «⋯», Shift+F10).
  function rowMenu(anchor, n) {
    const isFolder = !!n.children;
    const moveTo = [{ label: S.t('Top level'), icon: 'bookmark', disabled: parentOf(n.id) === null, action: () => S.post({ type: 'bookmark_move', id: n.id, parent: null }) }];
    for (const { f, depth } of folders(n.id)) {
      moveTo.push({ label: ' '.repeat(depth * 2) + folderName(f), icon: 'folder', disabled: f.children.includes(n),
        action: () => S.post({ type: 'bookmark_move', id: n.id, parent: f.id }) });
    }
    const siblings = (parentOf(n.id) === null ? items : find(parentOf(n.id)).children);
    const i = siblings.indexOf(n);
    const parent = parentOf(n.id);
    const order = [
      i > 0 ? { label: S.t('Move up'), icon: 'arrow-up', action: () => S.post({ type: 'bookmark_move', id: n.id, parent, index: i - 1 }) } : null,
      i < siblings.length - 1 ? { label: S.t('Move down'), icon: 'chevron-down', action: () => S.post({ type: 'bookmark_move', id: n.id, parent, index: i + 2 }) } : null,
    ];
    S.menu.open(anchor, isFolder ? [
      { label: S.t('Open all'), icon: 'external', disabled: !n.children.some((c) => !c.children), action: () => S.post({ type: 'bookmark_open_all', id: n.id }) },
      { label: S.t('New folder'), icon: 'folder-plus', action: () => { expanded.add(n.id); S.post({ type: 'bookmark_folder', parent: n.id, title: S.t('New folder') }); } },
      'sep',
      { label: S.t('Rename'), icon: 'pencil', kbd: 'F2', action: () => startEdit(n.id) },
      { label: S.t('Move to folder'), icon: 'folder', submenu: moveTo },
      ...order,
      'sep',
      { label: S.t('Delete'), icon: 'trash', kbd: 'Del', danger: true, action: () => S.post({ type: 'bookmark_remove', id: n.id }) },
    ] : [
      { label: S.t('Open'), icon: 'arrow-right', kbd: 'Enter', action: () => open(n, 'here') },
      { label: S.t('Open in new page'), icon: 'plus', action: () => open(n, 'new') },
      { label: S.t('Open in background'), icon: 'external', kbd: 'Ctrl Enter', action: () => open(n, 'background') },
      'sep',
      { label: S.t('Edit'), icon: 'pencil', kbd: 'F2', action: () => startEdit(n.id) },
      { label: S.t('Move to folder'), icon: 'folder', submenu: moveTo },
      ...order,
      'sep',
      { label: S.t('Delete'), icon: 'trash', kbd: 'Del', danger: true, action: () => S.post({ type: 'bookmark_remove', id: n.id }) },
    ]);
  }

  function row(n, depth, sub) {
    if (editing === n.id) return editor(n, depth);
    const i = rows.length;
    const isFolder = !!n.children;
    const open_ = isFolder && expanded.has(n.id);
    const more = S.iconBtn('more', S.t('More actions'), (e) => { e.stopPropagation(); rowMenu(e.currentTarget, n); }, 'sm');
    const el = S.h('div', {
      class: 'row' + (isFolder ? ' bm-folder' : ''),
      attrs: { role: 'treeitem', tabindex: '-1', 'aria-level': String(depth + 1), 'aria-expanded': isFolder && !sub ? String(open_) : null },
      style: { paddingLeft: 10 + depth * 16 + 'px' }, data: { id: n.id },
    },
    isFolder && !sub ? S.h('span', { class: 'bm-chev' + (open_ ? ' open' : '') }, S.icon('chevron-right', 'i12')) : !sub ? S.h('span', { class: 'bm-chev' }) : null,
    isFolder ? S.h('span', { class: 'ico' }, S.icon('folder', 'i16')) : S.fav({ url: n.url, title: n.title, favicon: icon(n.url) }),
    S.h('span', { class: 'row-main' }, S.h('span', { class: 'row-title', text: n.title || (isFolder ? S.t('Untitled folder') : n.url) }),
      S.h('span', { class: 'row-sub', text: sub || (isFolder ? '' : S.host(n.url)) })),
    isFolder ? S.h('span', { class: 'row-meta hide-on-hover', text: String(count(n)) }) : null,
    S.h('span', { class: 'row-acts' }, more));
    el.title = isFolder ? '' : n.url;
    el.addEventListener('click', (e) => activate(n, e.ctrlKey || e.metaKey ? 'background' : e.shiftKey ? 'new' : 'here'));
    el.addEventListener('auxclick', (e) => { if (e.button === 1 && !isFolder) { e.preventDefault(); open(n, 'background'); } });
    el.addEventListener('contextmenu', (e) => { e.preventDefault(); sel = i; mark(false); rowMenu(e, n); });
    el.addEventListener('mousemove', () => { if (sel !== i) { sel = i; mark(false); } });
    rows.push({ el, n, depth });
    return el;
  }

  function draw() {
    title.textContent = S.t('Bookmarks');
    for (const [b, l] of [[folderBtn, S.t('New folder')], [importBtn, S.t('Import from Chrome or Edge')], [closeBtn, S.t('Close')]]) { b.title = l; b.setAttribute('aria-label', l); }
    drawAdd();
    rows = [];
    const out = [];
    const q = search.value.trim().toLowerCase();
    if (q) {
      // Поиск по названию, адресу и папке — плоским списком с путём.
      const hits = [];
      const walk = (nodes, path) => {
        for (const n of nodes) {
          const name = (n.title || '').toLowerCase();
          if (n.children) {
            if (name.includes(q)) hits.push([n, path]);
            walk(n.children, path ? `${path} › ${n.title}` : n.title);
          } else if (name.includes(q) || n.url.toLowerCase().includes(q) || path.toLowerCase().includes(q)) hits.push([n, path]);
        }
      };
      walk(items, '');
      for (const [n, path] of hits.slice(0, 300)) out.push(row(n, 0, n.children ? (path || S.t('Top level')) : [S.host(n.url), path].filter(Boolean).join(' · ')));
      if (!hits.length) out.push(S.h('div', { class: 'empty' }, S.h('b', { text: S.t('Nothing found') }), S.t('Try a different word.')));
    } else {
      const walk = (nodes, depth) => {
        for (const n of nodes) {
          out.push(row(n, depth));
          if (n.children && expanded.has(n.id)) {
            if (n.children.length) walk(n.children, depth + 1);
            else out.push(S.h('div', { class: 'bm-empty-folder', style: { paddingLeft: 40 + (depth + 1) * 16 + 'px' }, text: S.t('This folder is empty') }));
          }
        }
      };
      walk(items, 0);
      if (!items.length) {
        const p = current();
        out.push(S.h('div', { class: 'empty' }, S.h('b', { text: S.t('No bookmarks yet') }),
          p ? S.t('Save this page — or press {0} on any site.', S.keyOf('bookmark') || 'Ctrl D') : S.t('Press {0} on any site to bookmark it.', S.keyOf('bookmark') || 'Ctrl D'),
          S.h('div', { class: 'bm-empty-acts' },
            p ? S.h('button', { class: 'btn primary', text: S.t('Bookmark this page'), on: { click: () => S.post({ type: 'bookmark_add', url: p.url, title: S.titleOf(p), parent: null }) } }) : null,
            S.h('button', { class: 'btn', text: S.t('Import from Chrome or Edge'), on: { click: openImport } }))));
      }
    }
    S.fill(list, ...out);
    sel = Math.min(sel, Math.max(0, rows.length - 1));
    mark(false);
  }
  function mark(scroll = true) {
    rows.forEach((r, i) => { r.el.classList.toggle('sel', i === sel); r.el.setAttribute('aria-selected', i === sel ? 'true' : 'false'); });
    S.activeOption(search, rows[sel] && rows[sel].el);
    if (scroll && rows[sel]) rows[sel].el.scrollIntoView({ block: 'nearest' });
  }
  // Где открыть: 'here' — в текущей странице (нет страницы — новой), 'new', 'background'.
  function open(n, where) {
    if (where !== 'background') S.layers.close('bookmarks');
    const mode = where === 'background' ? 'background' : where === 'new' || !S.activePage() ? 'new' : 'current';
    S.post({ type: 'navigate', text: n.url, mode });
  }
  function toggleFolder(n, on = !expanded.has(n.id)) {
    if (on) expanded.add(n.id); else expanded.delete(n.id);
    draw();
  }
  function activate(n, where) {
    if (n.children) {
      if (search.value.trim()) { search.value = ''; reveal(n.id); toggleFolder(n, true); }
      else toggleFolder(n);
    } else open(n, where);
  }
  function openImport() {
    S.layers.close('bookmarks');
    S.layers.open('settings', { section: 'import' });
  }

  search.addEventListener('input', () => { sel = 0; editing = null; draw(); });
  panel.addEventListener('keydown', (e) => {
    if (S.menu.isOpen() || editing !== null) return;
    const inSearch = document.activeElement === search;
    const r = rows[sel];
    if (e.key === 'ArrowDown') sel = Math.min(rows.length - 1, sel + 1);
    else if (e.key === 'ArrowUp') sel = Math.max(0, sel - 1);
    else if (e.key === 'Enter' && r) { activate(r.n, e.ctrlKey || e.altKey ? 'background' : e.shiftKey ? 'new' : 'here'); e.preventDefault(); return; }
    else if (e.key === 'Delete' && r && !(inSearch && search.value)) S.post({ type: 'bookmark_remove', id: r.n.id });
    else if (e.key === 'F2' && r) { startEdit(r.n.id); e.preventDefault(); return; }
    else if ((e.key === 'ContextMenu' || (e.key === 'F10' && e.shiftKey)) && r) { e.preventDefault(); rowMenu(r.el, r.n); return; }
    // → раскрыть папку, ← свернуть или к родительской папке (в пустом поиске).
    else if (e.key === 'ArrowRight' && r && r.n.children && !(inSearch && search.value)) { if (!expanded.has(r.n.id)) toggleFolder(r.n, true); else sel = Math.min(rows.length - 1, sel + 1); }
    else if (e.key === 'ArrowLeft' && r && !(inSearch && search.value)) {
      if (r.n.children && expanded.has(r.n.id)) toggleFolder(r.n, false);
      else { const p = parentOf(r.n.id); const i = rows.findIndex((x) => x.n.id === p); if (i >= 0) sel = i; }
    } else return;
    e.preventDefault();
    mark();
  });

  listeners.push(() => {
    // Удалённую папку больше не раскрываем.
    for (const id of [...expanded]) if (!find(id)) expanded.delete(id);
    if (S.layers.isOpen('bookmarks') && editing === null) draw();
  });
  S.on('data:history', () => { if (S.layers.isOpen('bookmarks') && editing === null) draw(); });
  S.on('render', () => { drawButton(); if (S.layers.isOpen('bookmarks') && editing === null) drawAdd(); });
  S.on('layers', drawButton);

  S.layers.register('bookmarks', {
    pop: true,
    onEsc() {
      if (editing !== null) { editing = null; draw(); return false; }
      if (search.value) { search.value = ''; draw(); return false; }
      return true;
    },
    open() {
      // Фокус вернётся туда, откуда открыли с клавиатуры (кнопка на панели).
      returnTo = document.activeElement === btn ? btn : null;
      search.value = '';
      search.placeholder = S.t('Search bookmarks…');
      search.setAttribute('aria-label', S.t('Search bookmarks…'));
      panel.setAttribute('aria-label', S.t('Bookmarks'));
      sel = 0; editing = null;
      S.loadHistory();
      draw();
      S.show(panel);
      requestAnimationFrame(() => search.focus());
    },
    close() {
      editing = null;
      // Список устаревает, пока панель закрыта: при открытии он рисуется заново.
      S.hide(panel, () => S.fill(list));
      if (returnTo && returnTo.isConnected) returnTo.focus({ preventScroll: true });
      returnTo = null;
    },
  });
  btn.addEventListener('click', () => S.cmd('bookmarks'));
  S.on('command:bookmarks', () => S.layers.toggle('bookmarks'));

  // ---------- Окно закладки (Ctrl+D, ☆) ----------
  const dlg = S.h('div', { class: 'pop bm-dialog right', hidden: true, attrs: { role: 'dialog', 'data-hit': '22' } });
  S.$('#popovers').append(dlg);
  let page = null, lastFolder = null;

  function folderMenu(anchor, chosen, pick) {
    S.menu.open(anchor, [
      { label: S.t('Top level'), icon: chosen === null ? 'check' : 'bookmark', action: () => pick(null) },
      ...folders().map(({ f, depth }) => ({ label: ' '.repeat(depth * 2) + folderName(f), icon: chosen === f.id ? 'check' : 'folder', action: () => pick(f.id) })),
    ]);
  }

  function drawDialog() {
    const saved = byUrl(page.url);
    let parent = saved ? parentOf(saved.id) : (lastFolder !== null && find(lastFolder) ? lastFolder : null);
    const head = S.h('h2', { text: saved ? S.t('Edit bookmark') : S.t('Add bookmark'), id: 'bm-dialog-title' });
    const name = S.h('input', { type: 'text', value: saved ? saved.title : page.title, attrs: { spellcheck: 'false', 'aria-label': S.t('Name') } });
    const folderLabel = S.h('span', { class: 'ellipsis' });
    const folderBtn_ = S.h('button', { class: 'bm-folder-pick', attrs: { 'aria-haspopup': 'menu', 'aria-label': S.t('Folder') } }, S.icon('folder', 'i16'), folderLabel, S.icon('chevron-down', 'i12'));
    const showFolder = () => { folderLabel.textContent = parent === null ? S.t('Top level') : folderName(find(parent)); };
    showFolder();
    folderBtn_.addEventListener('click', () => folderMenu(folderBtn_, parent, (id) => { parent = id; showFolder(); name.focus(); }));
    const save = () => {
      lastFolder = parent;
      S.post({ type: 'bookmark_add', url: page.url, title: name.value, parent });
      S.layers.close('bookmark_page');
    };
    name.addEventListener('keydown', (e) => { if (e.key === 'Enter') { e.preventDefault(); save(); } });
    S.fill(dlg, head,
      S.h('label', { class: 'bm-field' }, S.h('span', { text: S.t('Name') }), name),
      S.h('div', { class: 'bm-field' }, S.h('span', { text: S.t('Folder') }), folderBtn_),
      S.h('div', { class: 'bm-dialog-acts' },
        saved ? S.h('button', { class: 'btn ghost danger', text: S.t('Remove'), on: { click: () => { S.layers.close('bookmark_page'); S.post({ type: 'bookmark_remove', id: saved.id }); } } }) : null,
        S.h('span', { class: 'spacer' }),
        S.h('button', { class: 'btn ghost', text: S.t('Cancel'), on: { click: () => S.layers.close('bookmark_page') } }),
        S.h('button', { class: 'btn primary', text: S.t('Save'), on: { click: save } })));
    dlg.setAttribute('aria-labelledby', 'bm-dialog-title');
    requestAnimationFrame(() => { name.focus(); name.select(); });
  }

  S.layers.register('bookmark_page', {
    pop: true,
    onEsc: () => !S.menu.isOpen(),
    open(p) {
      page = p;
      drawDialog();
      const star = S.$('#addr-star');
      const anchor = star && !star.hidden && star.offsetParent ? star : btn.offsetParent ? btn : S.$('#menu-btn');
      S.placeAt(dlg, anchor, { align: 'right', gap: 10 });
      S.show(dlg);
    },
    close() { S.hide(dlg); page = null; },
  });
  S.on('command:bookmark_page', (p) => S.layers.open('bookmark_page', p));
})();
