// home.js — New Page: спокойная страница, не панель управления. В центре — поиск,
// под ним 5–6 частых сайтов; слева внизу — время и дата, справа — сводка защиты.
// Видна, когда на экране нет страницы (Ctrl+T, закрыли последнюю). Esc — вернуться.
import { S } from './core.js';

(() => {
  const home = S.$('#home');
  const input = S.h('input', { attrs: { spellcheck: 'false', autocomplete: 'off', role: 'combobox', 'aria-expanded': 'false', 'aria-controls': 'home-suggest' } });
  const search = S.h('label', { class: 'home-search' }, S.icon('search', 'i20'), input, S.h('kbd', { class: 'home-kbd' }));
  const tiles = S.h('nav', { class: 'home-tiles', attrs: { 'aria-label': 'Quick destinations' } });
  const center = S.h('div', { class: 'home-center' }, search, tiles);
  const time = S.h('div', { class: 'home-time' });
  const date = S.h('div', { class: 'home-date' });
  const clock = S.h('div', { class: 'home-clock', attrs: { 'aria-live': 'off' } }, time, date);
  const privacy = S.h('button', { class: 'home-privacy' });
  const update = S.h('div', { class: 'home-update', hidden: true });
  home.append(center, clock, privacy, update);
  const list = S.h('div', { class: 'pop suggest', id: 'home-suggest', hidden: true, attrs: { role: 'listbox', 'data-hit': '22' } });
  S.$('#popovers').append(list);

  // Пока сайтов в истории мало — спокойные сайты по умолчанию.
  const DEFAULTS = [
    ['Google', 'https://www.google.com/'], ['YouTube', 'https://www.youtube.com/'], ['Notion', 'https://www.notion.so/'],
    ['GitHub', 'https://github.com/'], ['Figma', 'https://www.figma.com/'],
  ];
  const visible = () => !home.hidden;
  S.region.full(visible);

  let items = [], sel = 0;

  // Значок — только из кэша ядра (data:); из сети New Page ничего не грузит.
  function tile(name, url, favicon) {
    const ico = S.h('span', { class: 'tile-ico' });
    if (favicon && favicon.startsWith('data:image/')) ico.append(S.h('img', { src: favicon, alt: '', draggable: false }));
    else ico.append(S.h('span', { class: 'letter', text: name[0].toUpperCase() }));
    return S.h('button', { class: 'tile', title: url, on: { click: () => S.post({ type: 'navigate', text: url, mode: 'new' }) } },
      ico, S.h('span', { class: 'tile-name', text: name }));
  }

  function drawTiles(top) {
    const seen = new Set();
    const list = [];
    for (const s of top || []) {
      const name = s.host.replace(/^(m|en|ru)\./, '').replace(/\.(com|org|net|ru|io|dev|app|so|uz)$/, '');
      if (seen.has(name)) continue;
      seen.add(name);
      list.push([name.charAt(0).toUpperCase() + name.slice(1), s.url, s.favicon]);
    }
    for (const d of DEFAULTS) if (list.length < 5 && !list.some((x) => S.host(x[1]) === S.host(d[1]))) list.push(d);
    const plus = S.h('button', { class: 'tile', title: S.t('Open new page'), on: { click: () => input.focus() } },
      S.h('span', { class: 'tile-ico' }, S.icon('plus', 'i20')), S.h('span', { class: 'tile-name', text: S.t('Add') }));
    plus.setAttribute('aria-label', S.t('Search or enter a URL…'));
    S.fill(tiles, ...list.slice(0, 5).map(([n, u, f]) => tile(n, u, f)), plus);
  }

  function tick() {
    const d = new Date();
    time.textContent = d.toLocaleTimeString(S.cfg.lang, { hour: '2-digit', minute: '2-digit' });
    const s = d.toLocaleDateString(S.cfg.lang, { weekday: 'long', month: 'long', day: 'numeric' });
    date.textContent = s.charAt(0).toUpperCase() + s.slice(1);
  }
  setInterval(() => { if (visible()) tick(); }, 15000);

  function drawPrivacy(st) {
    const on = st.shield && st.shield.on;
    const n = on ? st.shield.trackersToday + st.shield.adsToday : 0;
    privacy.classList.toggle('off', !on);
    S.fill(privacy, S.icon(on ? 'shield-check' : 'shield', 'i20'),
      S.h('span', { style: { textAlign: 'left' } },
        S.h('b', { text: on ? S.t('Jiyda protects you') : S.t('Tracker protection is off') }),
        on ? (n ? S.t('{0} trackers and ads blocked today', n) : S.t('No trackers blocked yet today')) : S.t('Turn it on in Settings')));
    privacy.onclick = () => S.layers.open('settings', { section: 'privacy' });
    if (st.update) {
      update.hidden = false;
      S.fill(update, S.h('span', { text: st.updating ? S.t('Downloading Jiyda {0}…', st.update.version) : S.t('Jiyda {0} is available', st.update.version) }),
        S.h('button', { class: 'btn primary', text: S.t('Update'), disabled: st.updating, on: { click: () => S.cmd('update_now') } }),
        S.iconBtn('x', S.t('Later'), () => S.cmd('update_dismiss'), 'sm'));
    } else update.hidden = true;
  }

  function draw() {
    const q = input.value;
    items = q.trim() ? S.suggest(q) : [];
    S.fill(list, ...items.map((r, i) => S.suggestRow(r, i === sel, () => go(r))));
    input.setAttribute('aria-expanded', items.length ? 'true' : 'false');
    if (!items.length || !visible()) { list.hidden = true; list.classList.remove('open'); return; }
    list.hidden = false;
    list.style.width = search.offsetWidth + 'px';
    S.placeAt(list, search, { gap: 8 });
    list.classList.add('open');
  }
  function go(r) {
    input.value = '';
    draw();
    S.pickSuggestion(r, 'new');
  }

  input.addEventListener('input', () => { sel = 0; draw(); });
  input.addEventListener('keydown', (e) => {
    if (e.key === 'ArrowDown' && items.length) { e.preventDefault(); sel = Math.min(sel + 1, items.length - 1); draw(); }
    else if (e.key === 'ArrowUp' && items.length) { e.preventDefault(); sel = Math.max(0, sel - 1); draw(); }
    else if (e.key === 'Enter') {
      e.preventDefault();
      const text = input.value.trim();
      if (!text) return;
      go(items[sel] || { kind: 'go', text });
    } else if (e.key === 'Escape') {
      e.preventDefault();
      if (input.value) { input.value = ''; draw(); }
      else if (S.state.home && S.state.home.canReturn) S.post({ type: 'home_cancel' });
    }
  });
  input.addEventListener('focus', () => S.loadHistory());
  input.addEventListener('blur', () => setTimeout(() => { list.hidden = true; list.classList.remove('open'); }, 120));
  S.on('data:history', () => { if (document.activeElement === input) draw(); });

  S.on('render', (st) => {
    const show = !st.active && !st.fullscreen;
    const was = visible();
    home.hidden = !show;
    if (!show) { list.hidden = true; return; }
    input.placeholder = S.t('Search or enter a URL…');
    input.setAttribute('aria-label', S.t('Search or enter a URL…'));
    S.$('.home-kbd', search).textContent = S.keyOf('palette') || 'Ctrl K';
    tiles.setAttribute('aria-label', S.t('Quick destinations'));
    drawTiles(st.home && st.home.top);
    drawPrivacy(st);
    tick();
    // Фокус — в следующем кадре и только если к тому времени не открылся слой
    // (например, переключатель Spaces с переименованием нового Space).
    if (!was) requestAnimationFrame(() => { if (!S.layers.any()) input.focus(); });
  });
  S.on('command:new_tab', () => requestAnimationFrame(() => { input.value = ''; input.focus(); draw(); }));
})();
