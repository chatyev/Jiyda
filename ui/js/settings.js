// settings.js — Settings: системный интерфейс, поэтому здесь постоянная колонка разделов
// (General, Privacy & Security, Appearance, AI Assistant, Spaces, Shortcuts, Downloads,
// Advanced). Значения хранит ядро (config.json); каждое изменение — сообщение 'setting'.
import { S } from './core.js';

(() => {
  const SECTIONS = [
    ['general', 'General', 'sliders'],
    ['privacy', 'Privacy & Security', 'shield'],
    ['appearance', 'Appearance', 'palette'],
    ['ai', 'AI Assistant', 'sparkle'],
    ['spaces', 'Spaces', 'layers'],
    ['shortcuts', 'Shortcuts', 'keyboard'],
    ['downloads', 'Downloads', 'download'],
    ['advanced', 'Advanced', 'wrench'],
  ];
  const COMMAND_NAMES = {
    palette: 'Command palette', orbit: 'Orbit', edit_url: 'Focus address bar', new_tab: 'New page', close: 'Close page',
    reopen: 'Restore recently closed page', next: 'Switch to recent page', prev: 'Switch to recent page (backwards)',
    next_seq: 'Next page', prev_seq: 'Previous page', back: 'Back', forward: 'Forward', reload: 'Reload',
    hard_reload: 'Reload without cache', find: 'Find on page', focus: 'Focus Mode', spaces: 'Switch Space',
    history: 'History', closed: 'Recently closed', downloads: 'Downloads', later: 'Move page to Later', split: 'Split View',
    ask: 'Ask Jiyda', fullscreen: 'Full screen', pin: 'Pin page', copy_url: 'Copy page link', settings: 'Settings',
    shortcuts: 'Keyboard shortcuts', devtools: 'Developer tools',
  };
  const ENGINES = [
    ['Google', 'https://www.google.com/search?q=%s'],
    ['Bing', 'https://www.bing.com/search?q=%s'],
    ['DuckDuckGo', 'https://duckduckgo.com/?q=%s'],
    ['Brave Search', 'https://search.brave.com/search?q=%s'],
    ['Ecosia', 'https://www.ecosia.org/search?q=%s'],
    ['Yandex', 'https://yandex.ru/search/?text=%s'],
  ];

  const el = S.h('div', { class: 'settings', hidden: true, attrs: { role: 'dialog', 'aria-modal': 'true' } });
  const nav = S.h('nav', { attrs: { role: 'tablist', 'aria-orientation': 'vertical' } });
  const main = S.h('div', { class: 'settings-main' });
  el.append(nav, main);
  document.body.append(el);

  let section = 'general', cfg = null, recording = null, conflict = null, highlightOrigin = '';
  const set = (key, value) => S.post({ type: 'setting', key, value });
  S.on('data:settings', (c) => { cfg = c; if (S.layers.isOpen('settings')) draw(); });
  S.on('data:recorded', (combo) => {
    if (!recording) return;
    const cmd = recording;
    recording = null;
    conflict = null;
    set('shortcut', { cmd, keys: combo });
    draw();
  });
  S.on('data:shortcut_conflict', (c) => { conflict = c; draw(); });

  // ---------- Строительные блоки ----------
  const card = (...rows) => S.h('div', { class: 'setting-card' }, ...rows);
  const row = (title, desc, control) => S.h('div', { class: 'setting' },
    S.h('div', { class: 'text' }, S.h('div', { class: 'title', text: title }), desc ? S.h('div', { class: 'desc', text: desc }) : null), control);
  const toggle = (key, value, label) => S.h('button', {
    class: 'switch', attrs: { role: 'switch', 'aria-checked': value ? 'true' : 'false', 'aria-label': label },
    on: { click: () => set(key, !value) },
  });
  const select = (key, value, options, label, map = (v) => v) => {
    const s = S.h('select', { attrs: { 'aria-label': label }, on: { change: () => set(key, map(s.value)) } },
      ...options.map(([v, l]) => S.h('option', { value: String(v), text: l, selected: String(v) === String(value) })));
    return s;
  };
  const head = (title, lead) => [S.h('h2', { text: title }), lead ? S.h('p', { class: 'lead', text: lead }) : null];
  const sub = (title) => S.h('h3', { text: title });

  // ---------- Разделы ----------
  function general(c) {
    const known = ENGINES.find(([, u]) => u === c.search_url);
    const custom = S.h('input', { type: 'text', value: known ? '' : c.search_url, placeholder: 'https://example.com/search?q=%s', attrs: { 'aria-label': S.t('Custom search address') } });
    custom.addEventListener('change', () => set('search_url', custom.value));
    const engine = S.h('select', { attrs: { 'aria-label': S.t('Search engine') }, on: { change: () => { if (engine.value !== 'custom') set('search_url', engine.value); else { draw(); } } } },
      ...ENGINES.map(([n, u]) => S.h('option', { value: u, text: n, selected: u === c.search_url })),
      S.h('option', { value: 'custom', text: S.t('Custom…'), selected: !known }));
    return [
      ...head(S.t('General'), S.t('Search, language and how Jiyda keeps memory use low.')),
      card(
        row(S.t('Search engine'), S.t('Used for searches from the address bar, New Page and the palette.'), engine),
        !known ? row(S.t('Custom search address'), S.t('Use %s where the query goes.'), custom) : null,
        row(S.t('Language'), S.t('Takes effect after restarting Jiyda.'), select('language', c.language, [['auto', S.t('Same as Windows')], ['en', 'English'], ['ru', 'Русский']], S.t('Language')))),
      sub(S.t('Pages')),
      card(
        row(S.t('Put inactive pages to sleep'), S.t('Background pages you haven’t opened for a while are unloaded from memory. Pages playing sound stay awake.'),
          select('sleep_minutes', c.sleep_minutes, [[0, S.t('Never')], [15, S.t('After {0} min', 15)], [30, S.t('After {0} min', 30)], [60, S.t('After 1 hour')], [120, S.t('After {0} hours', 2)], [240, S.t('After {0} hours', 4)]], S.t('Sleep'), Number)),
        row(S.t('Move unused pages to Archive'), S.t('Checked when Jiyda starts. Pinned and Later pages are never archived.'),
          select('archive_hours', c.archive_hours, [[0, S.t('Never')], [4, S.t('After {0} hours', 4)], [12, S.t('After {0} hours', 12)], [24, S.t('After 1 day')], [72, S.t('After {0} days', 3)], [168, S.t('After 1 week')]], S.t('Archive'), Number))),
      sub(S.t('Windows')),
      card(
        row(S.t('Default browser'), c.default_browser ? S.t('Open Windows Settings to choose Jiyda for links.') : S.t('Install Jiyda with its installer to make it available as a default browser.'),
          S.h('button', { class: 'btn', text: S.t('Open Windows Settings'), on: { click: () => S.cmd('default_browser') } })),
        row(S.t('Check for updates automatically'), S.t('Jiyda {0}. Updates come from GitHub Releases.', c.version), S.h('span', { style: { display: 'flex', gap: '12px', alignItems: 'center' } },
          S.h('button', { class: 'btn ghost', text: S.t('Check now'), on: { click: () => S.cmd('check_updates') } }),
          toggle('check_updates', c.check_updates, S.t('Check for updates automatically'))))),
    ];
  }

  function privacy(c, st) {
    const sites = Object.entries(c.permissions || {});
    sites.sort(([a], [b]) => (b === highlightOrigin) - (a === highlightOrigin) || a.localeCompare(b));
    return [
      ...head(S.t('Privacy & Security'), S.t('Jiyda protects you by default — you shouldn’t have to manage it.')),
      card(
        row(S.t('Block trackers and ads'), c.shield ? S.t('Blocked today: {0} trackers, {1} ads.', st.shield.trackersToday, st.shield.adsToday) : S.t('Known trackers and ad networks are allowed to load.'),
          toggle('shield', c.shield, S.t('Block trackers and ads'))),
        row(S.t('Cookies and logins'), S.t('Each Space has its own cookies, logins and cache, so sites in one Space can’t see another.'), S.h('span', { class: 'chip', text: S.t('Isolated per Space') })),
        row(S.t('Secure connections'), S.t('Sites with invalid certificates are blocked. Pages without HTTPS are marked in the address bar.'), S.h('span', { class: 'chip', text: S.t('On') }))),
      sub(S.t('Protection is off on these sites')),
      card(...(c.shield_allow.length ? c.shield_allow.map((site) => row(site, null,
        S.h('button', { class: 'btn ghost', text: S.t('Turn protection back on'), on: { click: () => S.post({ type: 'site_shield', host: site, on: true }) } })))
        : [S.h('div', { class: 'setting' }, S.h('div', { class: 'desc', text: S.t('None. You can turn protection off for a site from the shield in the address bar.') }))])),
      sub(S.t('Site permissions')),
      card(...(sites.length ? sites.map(([origin, kinds]) => S.h('div', { class: 'perm-site', style: origin === highlightOrigin ? { background: 'var(--sel)' } : null },
        S.h('div', { class: 'origin', text: origin }),
        S.h('div', { class: 'chips' }, ...Object.entries(kinds).map(([kind, d]) => S.h('span', { class: 'chip ' + d },
          S.t(S.permissionKind(kind)[1]), ' · ', d === 'allow' ? S.t('Allowed') : S.t('Blocked'),
          S.h('button', { title: S.t('Reset'), attrs: { 'aria-label': S.t('Reset') }, on: { click: () => S.post({ type: 'permission_revoke', origin, kind }) } }, S.icon('x', 'i12')))))))
        : [S.h('div', { class: 'setting' }, S.h('div', { class: 'desc', text: S.t('No saved decisions. When a site asks, “Allow once” never saves anything.') }))])),
      sub(S.t('Clear browsing data')),
      card(
        row(S.t('Browsing history'), S.t('Visited pages in all Spaces.'), S.h('button', { class: 'btn danger', text: S.t('Clear…'), on: { click: () => S.palette('confirm', { what: 'data' }) } })),
        row(S.t('Cookies, cache and saved passwords'), S.t('For the current Space “{0}”.', S.space().name), S.h('button', { class: 'btn danger', text: S.t('Clear…'), on: { click: () => S.palette('confirm', { what: 'data' }) } }))),
    ];
  }

  function appearance(c) {
    const themes = [['system', S.t('System')], ['light', S.t('Light')], ['dark', S.t('Dark')]];
    return [
      ...head(S.t('Appearance'), S.t('Jiyda follows Windows by default and keeps its own calm look.')),
      card(S.h('div', { class: 'setting' },
        S.h('div', { class: 'text' }, S.h('div', { class: 'title', text: S.t('Theme') }), S.h('div', { class: 'desc', text: S.t('“System” switches with Windows dark and light mode.') })),
        S.h('div', { class: 'theme-picker', attrs: { role: 'radiogroup', 'aria-label': S.t('Theme') } }, ...themes.map(([k, l]) => S.h('button', {
          attrs: { role: 'radio', 'aria-checked': c.theme === k ? 'true' : 'false' }, on: { click: () => set('theme', k) },
        }, S.h('span', { class: 'sw ' + k }), l))))),
      sub(S.t('While reading')),
      card(
        row(S.t('Dim the toolbar while reading'), S.t('Buttons fade until you move the pointer to the top.'), toggle('dim_chrome', c.dim_chrome, S.t('Dim the toolbar while reading'))),
        row(S.t('Edge Reveal'), S.t('Navigation appears when the pointer touches the left edge of the window.'), toggle('edge_reveal', c.edge_reveal, S.t('Edge Reveal'))),
        row(S.t('Toolbar for selected text'), S.t('Copy, Search, Explain, Translate and Ask Jiyda above selected text on pages.'), toggle('selection_toolbar', c.selection_toolbar, S.t('Toolbar for selected text')))),
      sub(S.t('Accessibility')),
      card(
        row(S.t('Animations'), S.t('“System” follows the Windows setting “Animation effects”.'), select('motion', c.motion, [['system', S.t('System')], ['reduced', S.t('Reduced')]], S.t('Animations'))),
        row(S.t('High contrast'), S.t('Jiyda uses Windows contrast themes automatically.'), S.h('span', { class: 'chip', text: S.t('Automatic') }))),
    ];
  }

  function ai(c) {
    const key = S.h('input', { type: 'password', placeholder: c.ai_key ? '••••••••••••••••' : 'sk-ant-…', attrs: { 'aria-label': S.t('Anthropic API key'), autocomplete: 'off' } });
    const save = S.h('button', { class: 'btn primary', text: S.t('Save key'), on: { click: () => { if (key.value.trim()) S.post({ type: 'ai_key', key: key.value }); } } });
    key.addEventListener('keydown', (e) => { if (e.key === 'Enter') save.click(); });
    return [
      ...head(S.t('AI Assistant'), S.t('Ask Jiyda appears only when you call it — no permanent sidebar.')),
      S.h('div', { class: 'notice' }, S.icon('info', 'i16'), S.h('div', { text: S.t('Ask Jiyda uses Claude (claude-opus-5-5) by Anthropic with your own API key. Page text, selected text or page titles are sent to Anthropic only when you press an action or ask a question. The key is stored in Windows Credential Manager, not in Jiyda’s files.') })),
      card(
        row(S.t('Anthropic API key'), c.ai_key ? S.t('A key is saved.') : S.t('Create a key at console.anthropic.com.'), S.h('span', { style: { display: 'flex', gap: '8px' } }, key, save)),
        c.ai_key ? row(S.t('Remove key'), S.t('Ask Jiyda will stop working until you add a key again.'), S.h('button', { class: 'btn danger', text: S.t('Remove'), on: { click: () => S.post({ type: 'ai_key', key: '' }) } })) : null,
        row(S.t('Show Ask Jiyda'), S.t('The ✦ button, palette command and selection actions.'), toggle('ai', c.ai, S.t('Show Ask Jiyda')))),
    ];
  }

  function spaces(st) {
    const rows = st.spaces.map((s, i) => {
      const name = S.h('input', { type: 'text', value: s.name, attrs: { 'aria-label': S.t('Space name'), maxlength: '40' }, style: { width: '220px' } });
      name.addEventListener('change', () => S.post({ type: 'space_rename', index: i, name: name.value }));
      const color = S.h('button', { class: 'icon-btn sm', title: S.t('Color'), attrs: { 'aria-label': S.t('Color') }, on: { click: (e) => S.menu.open(e.currentTarget, [{ swatches: Object.entries(S.SPACE_COLORS).map(([k, col]) => ({ color: col, current: k === s.color, label: k, action: () => S.post({ type: 'space_color', index: i, color: k }) })) }]) } },
        S.h('span', { class: 'space-color', style: { background: S.colorOf(s.color) } }));
      return S.h('div', { class: 'setting' }, color,
        S.h('div', { class: 'text' }, name, S.h('div', { class: 'desc', text: S.t('{0} pages', s.pages) + (i === st.activeSpace ? ' · ' + S.t('current') : '') + (i < 9 ? ` · Ctrl Shift ${i + 1}` : '') })),
        s.main ? S.h('span', { class: 'chip', text: S.t('Main') }) : S.h('button', { class: 'btn danger', text: S.t('Delete…'), on: { click: () => S.layers.open('spaces', {}) } }));
    });
    return [
      ...head(S.t('Spaces'), S.t('Separate contexts for work, personal life, research… Each keeps its pages, Orbit groups, Split View, Later and its own cookies and logins.')),
      card(...rows),
      S.h('div', { style: { marginTop: '12px' } }, S.h('button', { class: 'btn', on: { click: () => S.post({ type: 'space_new' }) } }, S.icon('plus', 'i16'), S.t('New Space'))),
    ];
  }

  function shortcuts(c) {
    const rows = (c.keys || []).map((k) => {
      const rec = recording === k.cmd;
      const label = rec ? S.t('Press keys…') : k.keys.length ? k.keys.join(', ').replace(/\+/g, ' ') : S.t('None');
      const btn = S.h('button', { class: 'kbd-btn' + (rec ? ' recording' : '') + (k.keys.length ? '' : ' none'), text: label,
        attrs: { 'aria-label': `${S.t(COMMAND_NAMES[k.cmd] || k.cmd)}: ${label}. ${S.t('Press to change')}` },
        on: { click: () => startRecord(k.cmd) } });
      const changed = k.keys.join() !== k.default.join();
      return S.h('div', { class: 'setting', style: { minHeight: '48px' } },
        S.h('div', { class: 'text' }, S.h('div', { class: 'title', text: S.t(COMMAND_NAMES[k.cmd] || k.cmd) }),
          changed ? S.h('div', { class: 'desc', text: S.t('Default: {0}', k.default.join(', ').replace(/\+/g, ' ') || S.t('None')) }) : null),
        conflict && conflict.cmd === k.cmd ? S.h('span', { class: 'chip block', text: S.t('{0} is used by “{1}”', conflict.keys.replace(/\+/g, ' '), S.t(COMMAND_NAMES[conflict.owner] || conflict.owner)) }) : null,
        changed ? S.h('button', { class: 'btn ghost', text: S.t('Reset'), on: { click: () => set('shortcut_reset', k.cmd) } }) : null,
        k.keys.length ? S.iconBtn('x', S.t('Remove shortcut'), () => set('shortcut', { cmd: k.cmd, keys: '' }), 'sm') : null,
        btn);
    });
    return [
      ...head(S.t('Shortcuts'), S.t('Windows shortcuts work as you expect. Every Jiyda shortcut can be changed: click it and press a new combination with Ctrl or Alt (or an F-key). Ctrl+1…9 open pages, Ctrl+Shift+1…9 switch Spaces.')),
      card(...rows),
      S.h('div', { style: { marginTop: '12px' } }, S.h('button', { class: 'btn', text: S.t('Reset all shortcuts'), on: { click: () => set('shortcut_reset', null) } })),
    ];
  }
  function startRecord(cmd) {
    recording = cmd;
    conflict = null;
    S.post({ type: 'keys_record', on: true });
    draw();
  }

  function downloads(c) {
    const dir = S.h('input', { type: 'text', value: c.download_dir, attrs: { 'aria-label': S.t('Download folder') }, style: { width: '340px' } });
    dir.addEventListener('change', () => set('download_dir', dir.value));
    return [
      ...head(S.t('Downloads'), S.t('Downloads show as a small card in the corner; the full list opens with {0}.', S.keyOf('downloads') || 'Ctrl J')),
      card(
        row(S.t('Save files to'), S.t('Paste a folder path. Leave empty to use the Windows Downloads folder.'), dir),
        row(S.t('Open the folder'), null, S.h('button', { class: 'btn', text: S.t('Open folder'), on: { click: () => S.cmd('open_downloads_folder') } }))),
    ];
  }

  function advanced(c) {
    return [
      ...head(S.t('Advanced'), S.t('Changes here take effect after restarting Jiyda.')),
      card(
        row(S.t('Jiyda title bar'), S.t('Jiyda’s own top bar with Windows buttons and Snap Layouts. Turn off to use the standard Windows frame.'), toggle('custom_titlebar', c.custom_titlebar, S.t('Jiyda title bar'))),
        row(S.t('Extensions'), S.t('Load unpacked extensions from the extensions folder in Jiyda’s data folder.'), toggle('extensions', c.extensions, S.t('Extensions'))),
        row(S.t('Developer tools'), S.t('Allow F12 and Ctrl+Shift+I.'), toggle('devtools', c.devtools, S.t('Developer tools')))),
      sub(S.t('About')),
      card(
        row('Jiyda ' + c.version, S.t('Rust core, WebView2 engine.'), S.h('button', { class: 'btn ghost', text: S.t('Check for updates'), on: { click: () => S.cmd('check_updates') } })),
        row(S.t('Data folder'), c.data_dir, S.h('button', { class: 'btn', text: S.t('Open folder'), on: { click: () => S.cmd('open_data_folder') } }))),
    ];
  }

  function draw() {
    const st = S.state;
    S.fill(nav, S.h('h1', { text: S.t('Settings') }), ...SECTIONS.map(([k, l, ic]) => S.h('button', {
      attrs: { role: 'tab', 'aria-selected': section === k ? 'true' : 'false' },
      on: { click: () => { section = k; recording = null; S.post({ type: 'keys_record', on: false }); draw(); main.scrollTop = 0; } },
    }, S.icon(ic), S.t(l))));
    el.setAttribute('aria-label', S.t('Settings'));
    if (!cfg) { S.fill(main, S.h('div', { class: 'settings-content' })); return; }
    const c = cfg;
    const body = { general: () => general(c), privacy: () => privacy(c, st), appearance: () => appearance(c), ai: () => ai(c), spaces: () => spaces(st), shortcuts: () => shortcuts(c), downloads: () => downloads(c), advanced: () => advanced(c) }[section]();
    const close = S.h('div', { style: { position: 'absolute', right: '20px', top: '16px' } }, S.iconBtn('x', S.t('Close settings (Esc)'), () => S.layers.close('settings')));
    S.fill(main, close, S.h('div', { class: 'settings-content', attrs: { role: 'tabpanel' } }, ...body));
  }

  // Клавиатура в колонке разделов: ↑↓.
  nav.addEventListener('keydown', (e) => {
    const i = SECTIONS.findIndex(([k]) => k === section);
    if (e.key === 'ArrowDown') section = SECTIONS[(i + 1) % SECTIONS.length][0];
    else if (e.key === 'ArrowUp') section = SECTIONS[(i - 1 + SECTIONS.length) % SECTIONS.length][0];
    else return;
    e.preventDefault();
    draw();
    S.$('[aria-selected="true"]', nav).focus();
  });

  S.layers.register('settings', {
    full: true,
    onEsc() {
      if (recording) { recording = null; S.post({ type: 'keys_record', on: false }); draw(); return false; }
      return true;
    },
    open(a = {}) {
      if (a.section) section = a.section;
      highlightOrigin = a.origin || '';
      el.hidden = false;
      S.post({ type: 'get', what: 'settings' });
      draw();
      requestAnimationFrame(() => { el.classList.add('open'); const b = S.$('[aria-selected="true"]', nav); if (b) b.focus(); });
    },
    close() {
      if (recording) { recording = null; S.post({ type: 'keys_record', on: false }); }
      S.hide(el);
    },
  });
  // Состояние браузера обновляется часто (заголовки страниц) — не перерисовываем, пока пользователь печатает.
  const typing = () => el.contains(document.activeElement) && /^(INPUT|SELECT)$/.test(document.activeElement.tagName);
  S.on('render', () => { if (S.layers.isOpen('settings') && !recording && !typing()) draw(); });
  S.on('command:settings', () => S.layers.toggle('settings'));
  S.on('command:shortcuts', () => S.layers.open('settings', { section: 'shortcuts' }));
})();
