// titlebar.js — верхняя панель: Space, назад/вперёд, Ask Jiyda, меню браузера.
// Адресная строка — addressbar.js. Кнопки окна справа рисует само окно (src/frame.rs),
// здесь под них только оставлено место (.caption-space).
import { S } from './core.js';

(() => {
  const chrome = S.$('#chrome');
  const spaceBtn = S.$('#space-btn');

  // Пустое место панели — как заголовок окна Windows: потянуть — перенести окно (с прилипанием
  // к краям), двойной щелчок — развернуть или восстановить, правый щелчок — системное меню.
  // Делает ядро: CSS app-region в WebView2 может остановить главный поток (см. frame.rs).
  const interactive = 'button, input, textarea, select, a, [role="button"], .no-drag';
  const empty = (e) => !e.target.closest(interactive);
  chrome.addEventListener('mousedown', (e) => {
    if (e.button !== 0 || !empty(e)) return;
    e.preventDefault();
    if (e.detail === 2) S.cmd('win_max');
    else if (e.detail === 1) S.post({ type: 'window_drag' });
  });
  chrome.addEventListener('contextmenu', (e) => {
    if (!empty(e)) return;
    e.preventDefault();
    S.post({ type: 'window_menu' });
  });

  // Подписи и подсказки с текущими сочетаниями клавиш.
  function labels() {
    const set = (el, label, cmd) => { const l = S.withKey(label, cmd); el.title = l; el.setAttribute('aria-label', l); };
    set(S.$('#back'), S.t('Back'), 'back');
    set(S.$('#forward'), S.t('Forward'), 'forward');
    set(S.$('#ask-btn'), S.t('Ask Jiyda'), 'ask');
    set(S.$('#menu-btn'), S.t('Browser menu'), null);
    set(spaceBtn, S.t('Switch Space'), 'spaces');
  }

  S.on('render', (st) => {
    const sp = S.space();
    S.$('.space-name', spaceBtn).textContent = sp.name;
    document.body.classList.toggle('fullscreen', !!st.fullscreen);
    const act = S.activePage();
    S.$('#back').disabled = !act;
    S.$('#forward').disabled = !act;
    labels();
  });

  for (const b of S.$$('[data-cmd]')) b.addEventListener('click', () => S.cmd(b.dataset.cmd));
  spaceBtn.addEventListener('click', () => S.layers.toggle('spaces', { anchor: spaceBtn }));
  S.$('#ask-btn').addEventListener('click', () => S.layers.toggle('ask', {}));
  S.$('#menu-btn').addEventListener('click', (e) => openMainMenu(e.currentTarget));

  // Меню браузера (⋯).
  function openMainMenu(anchor) {
    const act = S.activePage();
    const k = S.keyOf;
    S.menu.open(anchor, [
      { label: S.t('New page'), icon: 'plus', kbd: k('new_tab'), action: () => S.cmd('new_tab') },
      { label: S.t('Orbit'), icon: 'orbit', kbd: k('orbit'), action: () => S.layers.open('orbit') },
      { label: S.t('Command palette'), icon: 'command', kbd: k('palette'), action: () => S.layers.open('palette') },
      'sep',
      { label: S.t('History'), icon: 'history', kbd: k('history'), action: () => S.layers.open('history') },
      { label: S.t('Recently closed'), icon: 'undo', kbd: k('closed'), action: () => S.layers.open('closed') },
      { label: S.t('Downloads'), icon: 'download', kbd: k('downloads'), action: () => S.cmd('downloads') },
      'sep',
      { label: S.t('Find on page'), icon: 'search', kbd: k('find'), disabled: !act, action: () => S.cmd('find') },
      { label: S.t('Split View'), icon: 'columns', kbd: k('split'), disabled: !act, action: () => S.cmd('split') },
      { label: S.t('Focus Mode'), icon: 'focus', kbd: k('focus'), action: () => S.emit('command:focus') },
      { label: S.t('Full screen'), icon: 'maximize', kbd: k('fullscreen'), action: () => S.cmd('fullscreen') },
      'sep',
      { label: S.t('Settings'), icon: 'settings', kbd: k('settings'), action: () => S.layers.open('settings') },
      { label: S.t('Keyboard shortcuts'), icon: 'keyboard', kbd: k('shortcuts'), action: () => S.layers.open('settings', { section: 'shortcuts' }) },
      S.state.update ? 'sep' : null,
      S.state.update ? { label: S.t('Update to Jiyda {0}', S.state.update.version), icon: 'download', action: () => S.cmd('update_now') } : null,
    ]);
  }
  S.openMainMenu = () => openMainMenu(S.$('#menu-btn'));

  // ---------- Чтение: панель приглушается, пока мышь на странице ----------
  let restTimer = 0;
  function rest(on) {
    clearTimeout(restTimer);
    if (!on) { chrome.classList.remove('rest'); return; }
    restTimer = setTimeout(() => {
      const st = S.state;
      if (st.prefs && st.prefs.dimChrome && st.active && !S.layers.any() && !chrome.contains(document.activeElement)) {
        chrome.classList.add('rest');
      }
    }, 2200);
  }
  chrome.addEventListener('mouseenter', () => rest(false));
  chrome.addEventListener('mouseleave', () => rest(true));
  chrome.addEventListener('focusin', () => rest(false));
  S.on('render', (st) => { if (!st.active) rest(false); });
  S.on('layers', () => { if (S.layers.any()) rest(false); });

  // ---------- Полноэкранный режим: панель по наведению на верхний край ----------
  const strip = S.h('div', { class: 'top-strip', hidden: true, attrs: { 'data-hit': '' } });
  document.body.append(strip);
  let peekTimer = 0;
  function peek(on) {
    clearTimeout(peekTimer);
    if (on) { document.body.classList.add('chrome-peek'); chrome.setAttribute('data-hit', ''); }
    else {
      peekTimer = setTimeout(() => {
        if (chrome.matches(':hover') || chrome.contains(document.activeElement) || S.layers.any()) return;
        document.body.classList.remove('chrome-peek');
        chrome.removeAttribute('data-hit');
      }, 600);
    }
  }
  strip.addEventListener('mouseenter', () => peek(true));
  chrome.addEventListener('mouseleave', () => { if (document.body.classList.contains('fullscreen')) peek(false); });
  S.on('render', (st) => {
    strip.hidden = !st.fullscreen || st.pageFullscreen;
    if (!st.fullscreen) { document.body.classList.remove('chrome-peek'); chrome.removeAttribute('data-hit'); }
  });
  S.on('command:edit_url', () => { if (document.body.classList.contains('fullscreen')) peek(true); });
})();
