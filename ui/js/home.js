// home.js — New Page: спокойная точка продолжения работы, не каталог ярлыков. В центре —
// поиск, под ним до четырёх лёгких карточек из настоящих локальных данных: «Продолжить»,
// «Недавние», «Пространства», «Действия». Карточка без данных не показывается.
// Слева внизу — время и дата, справа — сводка защиты. Видна, когда на экране нет страницы
// (Ctrl+T, закрыли последнюю). Esc — вернуться. Подсветка — от цвета Space (styles.css, --glow).
import { S } from './core.js';

(() => {
  const home = S.$('#home');
  const input = S.h('input', { attrs: { spellcheck: 'false', autocomplete: 'off', role: 'combobox', 'aria-expanded': 'false', 'aria-controls': 'home-suggest' } });
  const search = S.h('label', { class: 'home-search' }, S.icon('search', 'i20'), input, S.h('kbd', { class: 'home-kbd' }));
  const cards = S.h('div', { class: 'home-cards' });
  // Первый запуск: найден Chrome или Edge — предложить перенести закладки и историю.
  const offer = S.h('div', { class: 'home-card home-import', hidden: true, attrs: { role: 'status' } });
  const main = S.h('div', { class: 'home-main' }, offer, search, cards);
  const time = S.h('div', { class: 'home-time' });
  const date = S.h('div', { class: 'home-date' });
  const clock = S.h('div', { class: 'home-clock', attrs: { 'aria-live': 'off' } }, time, date);
  const privacy = S.h('button', { class: 'home-privacy' });
  const update = S.h('div', { class: 'home-update', hidden: true });
  home.append(main, clock, privacy, update);
  const list = S.h('div', { class: 'pop suggest', id: 'home-suggest', hidden: true, attrs: { role: 'listbox', 'data-hit': '22' } });
  S.$('#popovers').append(list);

  const visible = () => !home.hidden;
  S.region.full(visible);

  let items = [], sel = 0;

  // ---------- Карточки ----------
  // Карточка: заголовок и до трёх строк-кнопок. Строка: значок, название, пояснение, справа —
  // сочетание клавиш или действие.
  function card(cls, icon, title, ...rows) {
    const id = 'hc-' + cls;
    return S.h('section', { class: 'hc hc-' + cls, attrs: { 'aria-labelledby': id } },
      S.h('h2', { class: 'hc-head', id }, S.icon(icon, 'i16'), S.h('span', { text: title })), ...rows);
  }
  function row({ ico, title, sub, kbd, label, onClick }) {
    return S.h('button', { class: 'hc-row', title: label || null, attrs: { 'aria-label': label || null }, on: { click: onClick } },
      ico,
      S.h('span', { class: 'hc-text' }, S.h('span', { class: 'hc-title', text: title }), sub ? S.h('span', { class: 'hc-sub', text: sub }) : null),
      kbd ? S.h('kbd', { text: kbd }) : null);
  }
  const glyph = (name) => S.h('span', { class: 'hc-ico' }, S.icon(name, 'i16'));
  const site = (url, title, favicon) => S.fav({ url, title, favicon });

  // «Продолжить»: после сбоя — вернуть страницы; иначе — последняя страница этого Space.
  function continueCard(st) {
    if (st.recovery) {
      return card('continue', 'history', S.t('Continue where you left off'),
        S.h('p', { class: 'hc-note' }, S.h('b', { text: S.t('The last session ended unexpectedly') }), S.t('Your pages are saved.')),
        S.h('div', { class: 'hc-acts' },
          S.h('button', { class: 'btn primary', text: S.t('Reopen pages'), on: { click: () => S.cmd('recover') } }),
          S.h('button', { class: 'btn ghost', text: S.t('Not now'), on: { click: () => S.cmd('recover_dismiss') } })));
    }
    const open = S.pagesHere().filter((p) => p.state === 'now').sort((a, b) => b.lastActiveAt - a.lastActiveAt);
    if (!open.length) return null;
    // Ушли на New Page со страницы (Ctrl+T) — она и есть «последняя»; время активности у
    // страниц, открытых в одну секунду, совпадает.
    const back = st.home && st.home.returnTo;
    const last = open.find((p) => p.id === back) || open[0];
    return card('continue', 'arrow-right', S.t('Continue where you left off'),
      row({ ico: S.fav(last), title: S.titleOf(last), sub: [last.domain, S.ago(last.lastActiveAt)].filter(Boolean).join(' · '),
        onClick: () => S.post({ type: 'activate', id: last.id }) }),
      open.length > 1 ? row({ ico: glyph('orbit'), title: S.t('All open pages'), sub: S.t('Open pages: {0}', open.length), kbd: S.keyOf('orbit'),
        onClick: () => S.layers.open('orbit') }) : null);
  }

  // «Недавние»: недавно закрытые (открываются со своей историей), затем сайты из истории.
  function recentCard(st) {
    const out = [];
    const seen = new Set();
    for (const c of st.closed || []) {
      if (out.length >= 3 || seen.has(c.url)) continue;
      seen.add(c.url);
      out.push(row({ ico: site(c.url, c.title, c.favicon), title: c.title || S.host(c.url), sub: [S.host(c.url), S.ago(c.closedAt)].filter(Boolean).join(' · '),
        onClick: () => S.post({ type: 'restore_closed', index: c.index }) }));
    }
    for (const e of (st.home && st.home.recent) || []) {
      if (out.length >= 3 || seen.has(e.url)) continue;
      seen.add(e.url);
      out.push(row({ ico: site(e.url, e.title, e.favicon), title: e.title || S.host(e.url), sub: [S.host(e.url), S.ago(e.ts)].filter(Boolean).join(' · '),
        onClick: () => S.post({ type: 'navigate', text: e.url, mode: 'new' }) }));
    }
    return out.length ? card('recent', 'clock', S.t('Recent'), ...out) : null;
  }

  // «Пространства»: до трёх Space со счётчиком страниц; один Space — предложить второй.
  function spacesCard(st) {
    const spaces = st.spaces || [];
    const rows = spaces.slice(0, spaces.length > 3 ? 2 : 3).map((s, i) => row({
      ico: S.h('span', { class: 'hc-dot', style: { background: S.colorOf(s.color) } }),
      title: s.name, sub: (i === st.activeSpace ? S.t('Current') + ' · ' : '') + S.t('Open pages: {0}', s.pages),
      // Текущий Space — открыть переключатель (переименовать, цвет), другой — перейти в него.
      onClick: () => (i === st.activeSpace ? S.layers.open('spaces', { anchor: S.$('#space-btn') }) : S.post({ type: 'space', index: i })),
    }));
    if (spaces.length > 3) rows.push(row({ ico: glyph('layers'), title: S.t('All Spaces'), sub: String(spaces.length), kbd: S.keyOf('spaces'), onClick: () => S.layers.open('spaces', { anchor: S.$('#space-btn') }) }));
    else if (spaces.length < 2) rows.push(row({ ico: glyph('plus'), title: S.t('New Space'), sub: S.t('Separate logins and pages for work or home'), onClick: () => S.post({ type: 'space_new' }) }));
    return card('spaces', 'layers', S.t('Spaces'), ...rows);
  }

  // «Действия»: только то, что работает. Ask Jiyda — когда ключ Anthropic API задан.
  function actionsCard(st) {
    const ai = st.prefs && st.prefs.aiKey;
    return card('actions', 'command', S.t('Actions'),
      row({ ico: glyph('bookmark'), title: S.t('Bookmarks'), kbd: S.keyOf('bookmarks'), onClick: () => S.cmd('bookmarks') }),
      row({ ico: glyph('history'), title: S.t('History'), kbd: S.keyOf('history'), onClick: () => S.layers.open('history') }),
      ai ? row({ ico: glyph('sparkle'), title: S.t('Ask Jiyda'), kbd: S.keyOf('ask'), onClick: () => S.layers.open('ask', {}) })
        : row({ ico: glyph('download'), title: S.t('Downloads'), kbd: S.keyOf('downloads'), onClick: () => S.cmd('downloads') }));
  }

  function drawCards(st) {
    // Не перерисовывать под мышью и под фокусом без нужды: состояние приходит часто.
    const list = [continueCard(st), recentCard(st), spacesCard(st), actionsCard(st)].filter(Boolean);
    const had = cards.contains(document.activeElement) ? [...cards.querySelectorAll('button')].indexOf(document.activeElement) : -1;
    S.fill(cards, ...list);
    cards.dataset.n = String(list.length);
    if (had >= 0) { const b = cards.querySelectorAll('button')[had]; if (b) b.focus({ preventScroll: true }); }
  }

  // ---------- Углы и подсказки ----------
  function tick() {
    const d = new Date();
    time.textContent = d.toLocaleTimeString(S.cfg.lang, { hour: '2-digit', minute: '2-digit' });
    const s = d.toLocaleDateString(S.cfg.lang, { weekday: 'long', month: 'long', day: 'numeric' });
    date.textContent = s.charAt(0).toUpperCase() + s.slice(1);
  }
  setInterval(() => { if (visible()) tick(); }, 15000);

  // Сводка защиты — из счётчиков ядра за сегодня (shield::Daily), без догадок.
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

  function drawOffer(st) {
    const o = !st.recovery && st.importOffer;
    offer.hidden = !o;
    if (!o) return;
    S.fill(offer, S.icon('bookmark', 'i20'),
      S.h('span', { class: 'home-card-text' },
        S.h('b', { text: S.t('Bring your bookmarks and history from {0}?', o.browsers.join(S.t(' or '))) }), S.t('Passwords stay in the other browser.')),
      S.h('button', { class: 'btn primary', text: S.t('Import'), on: { click: () => S.layers.open('settings', { section: 'import' }) } }),
      S.iconBtn('x', S.t('Not now'), () => S.cmd('import_dismiss'), 'sm'));
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
      // Сначала закрыть подсказки (текст остаётся), потом очистить, потом вернуться к странице.
      if (items.length) { items = []; list.hidden = true; list.classList.remove('open'); input.setAttribute('aria-expanded', 'false'); }
      else if (input.value) { input.value = ''; draw(); }
      else if (S.state.home && S.state.home.canReturn) S.post({ type: 'home_cancel' });
    }
  });
  input.addEventListener('focus', () => S.loadHistory());
  input.addEventListener('blur', () => setTimeout(() => { list.hidden = true; list.classList.remove('open'); }, 120));
  S.on('data:history', () => { if (document.activeElement === input) draw(); });
  // Подсказки привязаны к строке поиска: при прокрутке New Page — следом.
  main.addEventListener('scroll', () => { if (!list.hidden) S.placeAt(list, search, { gap: 8 }); }, { passive: true });

  S.on('render', (st) => {
    const show = !st.active && !st.fullscreen;
    const was = visible();
    home.hidden = !show;
    if (!show) { list.hidden = true; return; }
    input.placeholder = S.t('Search or enter a URL…');
    input.setAttribute('aria-label', S.t('Search or enter a URL…'));
    S.$('.home-kbd', search).textContent = S.keyOf('palette') || 'Ctrl K';
    drawCards(st);
    drawPrivacy(st);
    drawOffer(st);
    tick();
    // Фокус — в следующем кадре и только если к тому времени не открылся слой
    // (например, переключатель Spaces с переименованием нового Space).
    if (!was) requestAnimationFrame(() => { if (!S.layers.any()) input.focus(); });
  });
  S.on('command:new_tab', () => requestAnimationFrame(() => { input.value = ''; input.focus(); draw(); }));
})();
