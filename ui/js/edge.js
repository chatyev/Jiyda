// edge.js — Edge Reveal: навигация появляется у левого края окна.
// Курсор у края → тонкая подсказка; короткая задержка → панель 56px со значками:
// Orbit, Search, Spaces, History, Downloads, Later; ушёл — панель плавно прячется.
// В Focus Mode и при открытых слоях не появляется.
import { S } from './core.js';

(() => {
  const strip = S.$('#edge-strip');
  const panel = S.$('#edge');
  const OPEN_DELAY = 160, CLOSE_DELAY = 320;
  let openTimer = 0, closeTimer = 0, laterBadge = null;

  const item = (iconName, label, cmd, fn) => {
    const b = S.h('button', { class: 'icon-btn', attrs: { 'aria-label': label }, on: { click: () => { hide(true); fn(b); } } }, S.icon(iconName, 'i20'));
    b.dataset.label = label;
    b.dataset.cmd = cmd || '';
    return b;
  };
  const later = item('inbox', 'Later', 'later_list', () => S.layers.open('orbit', { tab: 'later' }));
  laterBadge = S.h('span', { class: 'badge', hidden: true });
  later.append(laterBadge);
  const items = [
    item('orbit', 'Orbit', 'orbit', () => S.layers.open('orbit')),
    item('search', 'Search', 'palette', () => S.layers.open('palette')),
    item('layers', 'Spaces', 'spaces', (b) => S.layers.open('spaces', { anchor: { left: 70, right: 70, top: b.getBoundingClientRect().top - 6, bottom: b.getBoundingClientRect().top - 6, width: 0, height: 0 } })),
    item('history', 'History', 'history', () => S.layers.open('history')),
    item('download', 'Downloads', 'downloads', () => S.cmd('downloads')),
    later,
  ];
  const settings = item('settings', 'Settings', 'settings', () => S.layers.open('settings'));
  panel.append(...items, S.h('div', { class: 'sep' }), settings);
  panel.setAttribute('data-hit', '22');
  strip.setAttribute('data-hit', '');

  function labels() {
    for (const b of [...items, settings]) {
      const l = S.withKey(S.t(b.dataset.label), b.dataset.cmd);
      b.title = l;
      b.setAttribute('aria-label', l);
    }
    panel.setAttribute('aria-label', S.t('Navigation'));
  }

  const enabled = () => {
    const st = S.state;
    return st.prefs && st.prefs.edgeReveal && !document.body.classList.contains('focus') && !S.layers.any() && !st.pageFullscreen;
  };

  function reveal() {
    if (!enabled()) return;
    clearTimeout(closeTimer);
    panel.hidden = false;
    requestAnimationFrame(() => panel.classList.add('open'));
    strip.classList.remove('hint');
  }
  function hide(now) {
    clearTimeout(openTimer);
    clearTimeout(closeTimer);
    const go = () => {
      strip.classList.remove('hint');
      if (panel.contains(document.activeElement)) document.activeElement.blur();
      S.hide(panel);
    };
    if (now) go(); else closeTimer = setTimeout(go, CLOSE_DELAY);
  }

  strip.addEventListener('mouseenter', () => {
    if (!enabled()) return;
    strip.classList.add('hint');
    clearTimeout(closeTimer);
    openTimer = setTimeout(reveal, OPEN_DELAY);
  });
  strip.addEventListener('mouseleave', () => { clearTimeout(openTimer); if (panel.hidden) strip.classList.remove('hint'); else hide(); });
  panel.addEventListener('mouseenter', () => clearTimeout(closeTimer));
  panel.addEventListener('mouseleave', () => hide());
  // Клавиатура: стрелки по значкам, Esc — спрятать.
  panel.addEventListener('keydown', (e) => {
    const all = [...items, settings];
    const i = all.indexOf(document.activeElement);
    if (e.key === 'ArrowDown') all[(i + 1) % all.length].focus();
    else if (e.key === 'ArrowUp') all[(i - 1 + all.length) % all.length].focus();
    else if (e.key === 'Escape') hide(true);
    else return;
    e.preventDefault();
  });

  S.on('render', (st) => {
    labels();
    strip.hidden = !(st.prefs && st.prefs.edgeReveal) || document.body.classList.contains('focus') || st.pageFullscreen;
    const n = st.pages.filter((p) => p.spaceId === S.space().id && p.state === 'later').length;
    laterBadge.hidden = !n;
    laterBadge.textContent = n > 99 ? '99+' : n;
    if (!enabled() && !panel.hidden) hide(true);
  });
  S.on('layers', () => { if (S.layers.any() && !panel.hidden) hide(true); });
  S.on('focusmode', (on) => { strip.hidden = on || !(S.state.prefs && S.state.prefs.edgeReveal); if (on) hide(true); });
  S.edge = { reveal, hide };
})();
