// palette.js — командная палитра (Ctrl+K): страницы, история, команды, Spaces.
// Режимы: обычный; 'split' — что открыть рядом; 'replace' — чем заменить половину
// Split View; 'confirm' — подтверждение необратимого (очистка данных).
import { S } from './core.js';

(() => {
  const layer = S.h('div', { class: 'layer light', hidden: true });
  const backdrop = S.h('div', { class: 'backdrop' });
  const input = S.h('input', { attrs: { spellcheck: 'false', autocomplete: 'off', role: 'combobox', 'aria-expanded': 'true', 'aria-controls': 'palette-list' } });
  const modeChip = S.h('span', { class: 'chip accent mode', hidden: true });
  const list = S.h('div', { class: 'panel-body', id: 'palette-list', attrs: { role: 'listbox' } });
  const foot = S.h('div', { class: 'palette-foot' });
  const panel = S.h('div', { class: 'panel palette', attrs: { role: 'dialog', 'aria-modal': 'true' } },
    S.h('div', { class: 'palette-field' }, S.icon('search', 'i20'), input, modeChip), list, foot);
  layer.append(backdrop, panel);
  S.$('#layers').append(layer);
  backdrop.addEventListener('mousedown', () => S.layers.close('palette'));

  let mode = 'default', arg = null, items = [], sel = 0;

  const page = () => !!S.activePage();
  const split = () => !!(S.state.active && S.splitOf(S.state.active));
  // Команды палитры. cmd — команда ядра (те же имена, что у горячих клавиш), fn — своя.
  const COMMANDS = () => [
    { t: 'Ask Jiyda', icon: 'sparkle', key: 'ask', fn: () => S.layers.open('ask', {}) },
    { t: 'Open new page', icon: 'plus', key: 'new_tab', cmd: 'new_tab' },
    { t: 'Search open pages', icon: 'orbit', key: 'orbit', fn: () => S.layers.open('orbit') },
    { t: 'Switch Space', icon: 'layers', key: 'spaces', fn: () => S.layers.open('spaces', {}) },
    { t: 'Show shortcuts', icon: 'keyboard', key: 'shortcuts', fn: () => S.layers.open('settings', { section: 'shortcuts' }) },
    { t: 'Open history', icon: 'history', key: 'history', fn: () => S.layers.open('history') },
    { t: 'Split current page', icon: 'columns', key: 'split', when: page, fn: () => open('split') },
    { t: 'Move page to Later', icon: 'inbox', key: 'later', when: page, cmd: 'later' },
    { t: 'Restore recently closed', icon: 'undo', key: 'closed', fn: () => S.layers.open('closed') },
    { t: 'Clear browsing data', icon: 'trash', fn: () => open('confirm', { what: 'data' }) },
    { t: 'Find on page', icon: 'search', key: 'find', when: page, cmd: 'find' },
    { t: 'Focus Mode', icon: 'focus', key: 'focus', fn: () => S.emit('command:focus') },
    { t: 'Full screen', icon: 'maximize', key: 'fullscreen', cmd: 'fullscreen' },
    { t: 'Downloads', icon: 'download', key: 'downloads', cmd: 'downloads' },
    { t: 'Settings', icon: 'settings', key: 'settings', fn: () => S.layers.open('settings') },
    { t: 'New Space', icon: 'layers', fn: () => S.post({ type: 'space_new' }) },
    { t: 'Swap sides of Split View', icon: 'swap', when: split, cmd: 'split_swap' },
    { t: 'Close Split View', icon: 'columns', when: split, cmd: 'unsplit' },
    { t: 'Copy page link', icon: 'link', key: 'copy_url', when: page, cmd: 'copy_url' },
    { t: 'Pin page', icon: 'pin', key: 'pin', when: page, cmd: 'pin' },
    { t: 'Reload page', icon: 'rotate-cw', key: 'reload', when: page, cmd: 'reload' },
    { t: 'Reload without cache', icon: 'rotate-cw', key: 'hard_reload', when: page, cmd: 'hard_reload' },
    { t: 'Close all pages in this Space', icon: 'x', fn: () => open('confirm', { what: 'close_all' }) },
    { t: 'Theme: same as Windows', icon: 'palette', fn: () => S.post({ type: 'setting', key: 'theme', value: 'system' }) },
    { t: 'Theme: light', icon: 'sun', fn: () => S.post({ type: 'setting', key: 'theme', value: 'light' }) },
    { t: 'Theme: dark', icon: 'moon', fn: () => S.post({ type: 'setting', key: 'theme', value: 'dark' }) },
    { t: 'Developer tools', icon: 'wrench', key: 'devtools', when: page, cmd: 'devtools' },
    { t: 'Check for updates', icon: 'download', cmd: 'check_updates' },
    { t: 'Make Jiyda the default browser', icon: 'globe', cmd: 'default_browser' },
    { t: 'Open downloads folder', icon: 'folder', cmd: 'open_downloads_folder' },
  ].filter((c) => !c.when || c.when());

  function cmdRow(c) {
    return { kind: 'cmd', title: S.t(c.t), icon: c.icon, kbd: c.key ? S.keyOf(c.key) : '', run: () => { if (c.fn) c.fn(); else S.cmd(c.cmd); } };
  }

  function build() {
    const q = input.value.trim();
    const ql = q.toLowerCase();
    const out = [];
    const add = (label, rows) => { if (rows.length) out.push({ label }, ...rows); };
    if (mode === 'confirm') {
      const what = arg.what;
      if (what === 'data') {
        add(S.t('Clear browsing data'), [
          { kind: 'cmd', title: S.t('Delete browsing history'), icon: 'history', danger: true, run: () => S.cmd('clear_history') },
          { kind: 'cmd', title: S.t('Delete cookies, cache and saved passwords in “{0}”', S.space().name), icon: 'cookie', danger: true, run: () => S.cmd('clear_data') },
          { kind: 'cmd', title: S.t('Cancel'), icon: 'x', run: () => {} },
        ]);
      } else {
        add(S.t('Close all pages in this Space?'), [
          { kind: 'cmd', title: S.t('Close all pages (pinned and Later stay)'), icon: 'x', danger: true, run: () => S.cmd('clear') },
          { kind: 'cmd', title: S.t('Cancel'), icon: 'x', run: () => {} },
        ]);
      }
      return out;
    }
    const pageRow = (p, verb) => ({ kind: 'page', page: p, title: S.titleOf(p), sub: p.domain, meta: verb, run: () => pickPage(p) });
    const mine = S.pagesHere().filter((p) => p.id !== S.state.active && (mode !== 'replace' || p.id !== arg.old));
    if (mode === 'split' || mode === 'replace') {
      const verb = mode === 'split' ? S.t('Open alongside') : S.t('Replace');
      if (q) {
        const go = S.suggest(q, { pages: 0, hist: 4 }).map((r) => ({ ...r, run: () => {
          if (mode === 'split') S.post({ type: 'navigate', text: r.text, mode: 'split' });
          else S.post({ type: 'navigate', text: r.text, mode: 'current' });
        } }));
        add(S.t('Open'), go.slice(0, 1));
        add(S.t('Pages'), mine.filter((p) => (p.title + ' ' + p.url).toLowerCase().includes(ql)).slice(0, 8).map((p) => pageRow(p, verb)));
        add(S.t('History'), go.slice(1));
      } else add(S.t('Pages'), mine.slice(0, 12).map((p) => pageRow(p, verb)));
      return out;
    }
    if (!q) {
      // Пустой запрос: недавние страницы и команды.
      const recent = [...mine].sort((a, b) => b.lastActiveAt - a.lastActiveAt).slice(0, 4);
      add(S.t('Recent pages'), recent.map((p) => pageRow(p, S.t('Switch'))));
      add(S.t('Commands'), COMMANDS().slice(0, 10).map(cmdRow));
      return out;
    }
    const sug = S.suggest(q, { pages: 6, hist: 6, exclude: S.state.active });
    add(S.t('Go to'), sug.filter((r) => r.kind === 'go' || r.kind === 'search').map((r) => ({ ...r, run: () => S.pickSuggestion(r, S.activePage() ? 'current' : 'new') })));
    add(S.t('Pages'), sug.filter((r) => r.kind === 'page').map((r) => ({ ...r, run: () => S.pickSuggestion(r) })));
    const cmds = COMMANDS().filter((c) => S.t(c.t).toLowerCase().includes(ql) || c.t.toLowerCase().includes(ql));
    add(S.t('Commands'), cmds.slice(0, 6).map(cmdRow));
    const spaces = S.state.spaces.map((s, i) => ({ s, i })).filter(({ s, i }) => i !== S.state.activeSpace && s.name.toLowerCase().includes(ql));
    add(S.t('Spaces'), spaces.map(({ s, i }) => ({ kind: 'space', title: s.name, icon: 'layers', meta: i < 9 ? `Ctrl Shift ${i + 1}` : '', dot: s.color, run: () => S.post({ type: 'space', index: i }) })));
    add(S.t('History'), sug.filter((r) => r.kind === 'history').map((r) => ({ ...r, run: () => S.pickSuggestion(r, 'new') })));
    return out;
  }

  function pickPage(p) {
    if (mode === 'split') S.post({ type: 'split', id: p.id });
    else if (mode === 'replace') S.post({ type: 'split_replace', old: arg.old, new: p.id });
    else S.post({ type: 'activate', id: p.id });
  }

  function draw() {
    items = build();
    const runnable = items.filter((x) => x.run);
    sel = Math.max(0, Math.min(sel, runnable.length - 1));
    let k = -1;
    S.fill(list, ...items.map((r) => {
      if (!r.run) return S.h('div', { class: 'group-label', text: r.label });
      k++;
      const i = k;
      const ico = r.page ? S.fav(r.page) : r.kind === 'history' ? S.fav({ url: r.url, title: r.title, favicon: r.favicon })
        : r.dot ? S.h('span', { class: 'ico' }, S.h('span', { class: 'space-dot', style: { '--space': S.colorOf(r.dot) } }))
        : S.h('span', { class: 'ico' }, S.icon(r.icon || 'globe', 'i16'));
      const el = S.h('button', { class: 'row' + (i === sel ? ' sel' : '') + (r.danger ? ' danger' : ''), attrs: { role: 'option', 'aria-selected': i === sel ? 'true' : 'false', tabindex: '-1' } },
        ico,
        S.h('span', { class: 'row-main' }, S.h('span', { class: 'row-title', text: r.title }), r.sub ? S.h('span', { class: 'row-sub', text: r.sub }) : null),
        r.kbd ? S.h('kbd', { text: r.kbd }) : r.meta ? S.h('span', { class: 'row-meta', text: r.meta }) : null);
      el.addEventListener('mousemove', () => { if (sel !== i) { sel = i; mark(); } });
      el.addEventListener('mousedown', (e) => e.preventDefault());
      el.addEventListener('click', () => run(i));
      return el;
    }));
    if (!runnable.length) list.append(S.h('div', { class: 'empty', text: S.t('Nothing found') }));
    S.activeOption(input, S.$$('.row', list)[sel]);
  }
  function mark() {
    S.$$('.row', list).forEach((el, i) => { el.classList.toggle('sel', i === sel); el.setAttribute('aria-selected', i === sel ? 'true' : 'false'); });
    const el = S.$$('.row', list)[sel];
    S.activeOption(input, el);
    if (el) el.scrollIntoView({ block: 'nearest' });
  }
  function run(i) {
    const r = items.filter((x) => x.run)[i];
    if (!r) return;
    S.layers.close('palette');
    r.run();
  }

  function open(m = 'default', a = null) {
    if (S.layers.isOpen('palette')) { setMode(m, a); return; }
    S.layers.open('palette', { mode: m, arg: a });
  }
  function setMode(m, a) {
    mode = m; arg = a;
    input.value = '';
    sel = 0;
    input.placeholder = {
      split: S.t('Open alongside: a page, address or search…'),
      replace: S.t('Replace with: a page, address or search…'),
      confirm: S.t('Confirm…'),
    }[mode] || S.t('Search pages, history, commands…');
    input.readOnly = mode === 'confirm';
    modeChip.hidden = mode === 'default';
    modeChip.textContent = { split: S.t('Split View'), replace: S.t('Split View'), confirm: S.t('Confirm') }[mode] || '';
    input.setAttribute('aria-label', input.placeholder);
    S.fill(foot, 
      S.h('span', {}, S.h('kbd', { text: '↑↓' }), S.t('Navigate')),
      S.h('span', {}, S.h('kbd', { text: 'Enter' }), S.t('Open')),
      mode === 'default' ? S.h('span', {}, S.h('kbd', { text: 'Alt+Enter' }), S.t('New page')) : null,
      S.h('span', {}, S.h('kbd', { text: 'Esc' }), S.t('Close')));
    draw();
  }
  S.palette = open;

  S.layers.register('palette', {
    full: true,
    open(a = {}) {
      layer.hidden = false;
      panel.setAttribute('aria-label', S.t('Command palette'));
      setMode(a.mode || 'default', a.arg || null);
      S.loadHistory();
      requestAnimationFrame(() => { layer.classList.add('open'); input.focus(); });
    },
    close() {
      S.hide(layer);
    },
  });
  input.addEventListener('input', () => { sel = 0; draw(); });
  input.addEventListener('keydown', (e) => {
    const n = items.filter((x) => x.run).length;
    if (e.key === 'ArrowDown') { sel = (sel + 1) % Math.max(1, n); mark(); }
    else if (e.key === 'ArrowUp') { sel = (sel - 1 + Math.max(1, n)) % Math.max(1, n); mark(); }
    else if (e.key === 'PageDown') { sel = Math.min(n - 1, sel + 8); mark(); }
    else if (e.key === 'PageUp') { sel = Math.max(0, sel - 8); mark(); }
    else if (e.key === 'Enter') {
      if (e.altKey && mode === 'default' && input.value.trim()) { S.layers.close('palette'); S.post({ type: 'navigate', text: input.value, mode: 'new' }); }
      else run(sel);
    } else return;
    e.preventDefault();
  });
  S.on('data:history', () => { if (S.layers.isOpen('palette')) draw(); });
  S.on('command:palette', () => { if (S.layers.isOpen('palette') && mode === 'default') S.layers.close('palette'); else open(); });
  S.on('command:split', () => open('split'));
})();
