// core.js — основа интерфейса: связь с ядром (Rust), состояние, слои, помощники.
//
// Интерфейс → ядро: S.post({type, ...}) — сообщения описаны в src/ipc.rs.
// Ядро → интерфейс: window.jiyda.render(state) — всё состояние браузера,
//   command(cmd, arg) — открыть палитру/Orbit/…, data(kind, payload) — списки по запросу,
//   toast(t), preview(id, uri), findResult(r).
// Компоненты (ui/js/*.js) подписываются на события S.on('render' | 'command:<cmd>' | 'data:<kind>' …).
const S = (() => {
  const cfg = Object.assign(
    { lang: 'en', chromeH: 46, captionW: 46, splitGap: 6, paneH: 30, frameless: true, version: '' },
    window.JIYDA_CONFIG || {},
  );
  const root = document.documentElement;
  root.style.setProperty('--chrome-h', cfg.chromeH + 'px');
  root.style.setProperty('--caption-w', cfg.captionW + 'px');
  root.style.setProperty('--split-gap', cfg.splitGap + 'px');
  root.style.setProperty('--pane-h', cfg.paneH + 'px');
  root.lang = cfg.lang;
  if (!cfg.frameless) document.body.classList.add('no-caption');

  // onPost — для проверки интерфейса (scripts/ui-smoke.ps1): видеть сообщения в ядро.
  let onPost = null;
  const post = (m) => {
    if (onPost) onPost(m);
    try { window.ipc.postMessage(JSON.stringify(m)); } catch (_) { /* вне Jiyda */ }
  };
  // Ошибки интерфейса — в журнал ядра (jiyda.log): DevTools у пользователя не открыты.
  addEventListener('error', (e) => post({ type: 'log', msg: `${e.message} @ ${e.lineno}:${e.colno}` }));
  addEventListener('unhandledrejection', (e) => post({ type: 'log', msg: 'promise: ' + String(e.reason) }));
  const $ = (s, r = document) => r.querySelector(s);
  const $$ = (s, r = document) => [...r.querySelectorAll(s)];

  // ---------- Перевод (словарь задаёт i18n.js) ----------
  // Ключ — английская строка; {0}, {1} — подстановки.
  let dict = null;
  const t = (s, ...args) => {
    let r = (dict && dict[s]) || s;
    args.forEach((a, i) => { r = r.split('{' + i + '}').join(a); });
    return r;
  };

  // ---------- Создание элементов ----------
  // h('button', { class: 'x', text: 'Hi', on: { click }, attrs: { 'aria-label': '…' } }, ...дети)
  function h(tag, props, ...kids) {
    const el = document.createElement(tag);
    if (props) {
      for (const [k, v] of Object.entries(props)) {
        if (v === undefined || v === null || v === false) continue;
        if (k === 'class') el.className = v;
        else if (k === 'text') el.textContent = v;
        else if (k === 'on') for (const [ev, fn] of Object.entries(v)) el.addEventListener(ev, fn);
        else if (k === 'attrs') for (const [a, av] of Object.entries(v)) { if (av !== undefined && av !== null && av !== false) el.setAttribute(a, av === true ? '' : av); }
        else if (k === 'style') Object.assign(el.style, v);
        else if (k === 'data') Object.assign(el.dataset, v);
        else el[k] = v;
      }
    }
    for (const kid of kids.flat()) {
      if (kid === null || kid === undefined || kid === false) continue;
      el.append(kid instanceof Node ? kid : document.createTextNode(String(kid)));
    }
    return el;
  }
  // Заменить детей элемента. Условные дети (cond ? el : null) — обычное дело,
  // а replaceChildren превратил бы null в текст «null», поэтому пустые пропускаем.
  // Так устроена вся отрисовка: слой перерисовывает содержимое целиком (это доли
  // миллисекунды даже на тысяче узлов), а сам контейнер остаётся — прокрутка не сбрасывается.
  // Теряются только фокус и набранный текст, поэтому слой с полем ввода не перерисовывается,
  // пока в поле печатают (см. settings.js, spaces.js).
  const fill = (el, ...kids) => el.replaceChildren(...kids.flat().filter((k) => k !== null && k !== undefined && k !== false));
  const icon = (name, cls = '') => {
    const ns = 'http://www.w3.org/2000/svg';
    const svg = document.createElementNS(ns, 'svg');
    svg.setAttribute('class', 'i ' + cls);
    svg.setAttribute('aria-hidden', 'true');
    const use = document.createElementNS(ns, 'use');
    use.setAttribute('href', '#i-' + name);
    svg.appendChild(use);
    return svg;
  };
  // Кнопка-значок с подсказкой и именем для экранного диктора.
  const iconBtn = (name, label, onClick, cls = '') =>
    h('button', { class: 'icon-btn ' + cls, title: label, attrs: { 'aria-label': label }, on: { click: onClick } }, icon(name));

  // ---------- События ----------
  const handlers = {};
  const on = (ev, fn) => { (handlers[ev] = handlers[ev] || []).push(fn); };
  const emit = (ev, ...args) => { for (const fn of handlers[ev] || []) { try { fn(...args); } catch (e) { console.error(ev, e); } } };

  // ---------- Состояние ----------
  let state = {
    pages: [], groups: [], active: null, visible: [], splits: [], spaces: [], activeSpace: 0, closed: [],
    theme: 'system', maximized: false, fullscreen: false, find: false, home: null,
    shield: { on: true, trackersToday: 0, adsToday: 0 }, keys: [], prefs: {}, peek: null,
  };
  const space = () => state.spaces[state.activeSpace] || { id: 0, name: '', color: 'blue', main: true, pages: 0 };
  const pagesHere = () => state.pages.filter((p) => p.spaceId === space().id);
  const page = (id) => state.pages.find((p) => p.id === id);
  const activePage = () => page(state.active);
  const splitOf = (id) => state.splits.find((s) => s.left === id || s.right === id);

  // Цвета пространств — ключи те же, что spaces::COLORS в Rust.
  const SPACE_COLORS = {
    violet: '#9B8CFF', blue: '#7CB7FF', teal: '#4FD1C5', green: '#5FD08A',
    orange: '#F2A65A', rose: '#F27A93', gray: '#9DA6B2',
  };
  const colorOf = (key) => SPACE_COLORS[key] || SPACE_COLORS.blue;

  // ---------- Тема ----------
  const darkSystem = matchMedia('(prefers-color-scheme: dark)');
  function applyTheme() {
    const th = state.theme || 'system';
    const dark = th === 'dark' || (th === 'system' && darkSystem.matches);
    root.dataset.theme = dark ? 'dark' : 'light';
    root.dataset.motion = state.prefs && state.prefs.motion === 'reduced' ? 'reduced' : '';
    root.style.setProperty('--space', colorOf(space().color));
    // Кнопки окна рисует само окно — сообщаем ему цвета панели.
    const cs = getComputedStyle(root);
    const hex = (v) => toHex(v.trim());
    const bg = hex(cs.getPropertyValue('--bg')), fg = hex(cs.getPropertyValue('--text'));
    const key = bg + fg;
    if (key !== applyTheme.last) { applyTheme.last = key; post({ type: 'frame_colors', bg, fg }); }
  }
  function toHex(v) {
    if (/^#[0-9a-f]{6}$/i.test(v)) return v.toLowerCase();
    const m = /rgba?\((\d+)\D+(\d+)\D+(\d+)/i.exec(v);
    return m ? '#' + [m[1], m[2], m[3]].map((x) => (+x).toString(16).padStart(2, '0')).join('') : '#0b0f14';
  }
  darkSystem.addEventListener('change', () => { applyTheme(); emit('theme'); });

  // ---------- Слои ----------
  // Слой — то, что открывается поверх страницы: Orbit, палитра, панели, меню Space…
  // def: { open(arg), close(), full: bool — закрывает всё окно, keyboard: bool — нужна
  //        клавиатура, pop: bool — всплывающая панель (закрывается при уходе фокуса) }.
  const reg = {};
  let stack = [];
  const layers = {
    register(name, def) { reg[name] = Object.assign({ keyboard: true, full: false, pop: false }, def); },
    isOpen: (name) => stack.includes(name),
    top: () => stack[stack.length - 1],
    any: () => stack.length > 0,
    open(name, arg) {
      const def = reg[name];
      if (!def) return;
      // Одновременно открыт один большой слой; всплывающие панели закрываются при любом новом слое.
      for (const other of [...stack]) {
        if (other !== name && (reg[other].pop || !def.pop)) layers.close(other, true);
      }
      if (!stack.includes(name)) stack.push(name);
      emit('layers');
      def.open(arg);
      syncModal();
      S.region.update();
    },
    close(name, quiet) {
      if (!stack.includes(name)) return;
      stack = stack.filter((n) => n !== name);
      reg[name].close();
      emit('layers');
      if (!quiet) syncModal();
      S.region.update();
    },
    toggle(name, arg) { if (layers.isOpen(name)) layers.close(name); else layers.open(name, arg); },
    closeAll() { for (const n of [...stack]) layers.close(n, true); syncModal(); },
    full: () => stack.some((n) => (typeof reg[n].full === 'function' ? reg[n].full() : reg[n].full)),
    closePops() { for (const n of [...stack]) if (reg[n].pop) layers.close(n, true); syncModal(); },
  };
  let modal = false;
  // Сообщаем ядру итог за кадр: «закрыли одно — открыли другое» не дёргает фокус туда-обратно.
  let modalTimer = 0;
  function syncModal() {
    clearTimeout(modalTimer);
    modalTimer = setTimeout(() => {
      const want = stack.some((n) => reg[n].keyboard) || extraModal > 0;
      if (want !== modal) { modal = want; post({ type: 'modal', on: want }); }
    }, 0);
  }
  // Клавиатура нужна не только слоям (например, поиску по странице или New Page).
  let extraModal = 0;
  const holdKeyboard = (on) => { extraModal = Math.max(0, extraModal + (on ? 1 : -1)); syncModal(); };

  // Esc закрывает верхний слой.
  addEventListener('keydown', (e) => {
    if (e.key === 'Escape' && stack.length && !e.defaultPrevented) {
      const top = stack[stack.length - 1];
      if (reg[top].onEsc && reg[top].onEsc(e) === false) return;
      e.preventDefault();
      layers.close(top);
    }
  });
  // Фокус ушёл. Сам ли пользователь ушёл (щелчок по странице, Alt+Tab) или фокус перевело
  // ядро — знает только ядро; если пользователь, оно пришлёт close_pops, и всплывающие
  // панели закроются, как в Windows.
  addEventListener('blur', () => post({ type: 'ui_blur' }));
  on('command:close_pops', () => layers.closePops());

  // Позиционировать всплывающую панель у элемента, не вылезая за окно.
  function placeAt(el, anchor, { align = 'left', gap = 6, dx = 0 } = {}) {
    const a = anchor instanceof Element ? anchor.getBoundingClientRect() : anchor;
    el.style.left = '0px'; el.style.top = '0px';
    // Скрытый элемент не имеет размера — меряем его невидимым.
    const wasHidden = el.hidden;
    if (wasHidden) { el.style.visibility = 'hidden'; el.hidden = false; }
    const r = el.getBoundingClientRect();
    if (wasHidden) { el.hidden = true; el.style.visibility = ''; }
    let x = align === 'right' ? a.right - r.width : align === 'center' ? a.left + a.width / 2 - r.width / 2 : a.left;
    x += dx;
    let y = a.bottom + gap;
    x = Math.max(8, Math.min(x, innerWidth - r.width - 8));
    if (y + r.height > innerHeight - 8) y = Math.max(8, a.top - r.height - gap);
    el.style.left = Math.round(x) + 'px';
    el.style.top = Math.round(y) + 'px';
  }

  // Показать/спрятать элемент с анимацией (класс open), с учётом hidden.
  function show(el) {
    el._want = true;
    el.hidden = false;
    // Сначала — в регион, потом анимация (иначе первые кадры обрезаны).
    void el.offsetWidth;
    requestAnimationFrame(() => { if (el._want) el.classList.add('open'); });
  }
  // Фокус остаётся в поле ввода, а экранный диктор читает выбранную строку списка.
  let optionSeq = 0;
  function activeOption(input, el) {
    if (el && !el.id) el.id = 'opt-' + ++optionSeq;
    if (el) input.setAttribute('aria-activedescendant', el.id);
    else input.removeAttribute('aria-activedescendant');
  }
  // Вызвать fn, когда закончится анимация закрытия (--t-large; при «Меньше движения» — почти сразу).
  function afterMotion(fn) {
    const ms = parseFloat(getComputedStyle(root).getPropertyValue('--t-large')) || 240;
    setTimeout(fn, ms + 20);
  }
  // Закрыть слой или панель: снять .open, а после анимации спрятать и убрать из региона.
  function hide(el, after) {
    el._want = false;
    el.classList.remove('open');
    afterMotion(() => { if (!el.classList.contains('open')) { el.hidden = true; if (after) after(); S.region.update(); } });
  }

  // ---------- Форматирование ----------
  function host(url) {
    try { const u = new URL(url); return u.hostname.replace(/^www\./, '') || url; } catch (_) { return url || ''; }
  }
  function splitUrl(url) {
    try {
      const u = new URL(url);
      if (!/^https?:$/.test(u.protocol)) return { host: url, rest: '' };
      const rest = (u.pathname === '/' ? '' : u.pathname) + u.search + u.hash;
      return { host: u.hostname.replace(/^www\./, ''), rest };
    } catch (_) { return { host: url || '', rest: '' }; }
  }
  const nowSec = () => Math.floor(Date.now() / 1000);
  function ago(ts) {
    if (!ts) return '';
    const d = Math.max(0, nowSec() - ts);
    if (d < 45) return t('just now');
    if (d < 3600) return t('{0} min ago', Math.round(d / 60));
    if (d < 86400) return t('{0} h ago', Math.round(d / 3600));
    if (d < 86400 * 7) return t('{0} d ago', Math.round(d / 86400));
    return new Date(ts * 1000).toLocaleDateString(cfg.lang, { day: 'numeric', month: 'short' });
  }
  function size(bytes) {
    if (!bytes) return '';
    const u = ['B', 'KB', 'MB', 'GB'];
    let i = 0, v = bytes;
    while (v >= 1024 && i < u.length - 1) { v /= 1024; i++; }
    return (v >= 10 || i === 0 ? Math.round(v) : v.toFixed(1)) + ' ' + u[i];
  }
  const time = (ts) => new Date(ts * 1000).toLocaleTimeString(cfg.lang, { hour: '2-digit', minute: '2-digit' });

  // Значок сайта: PNG от ядра (data:), иначе первая буква. Из сети интерфейс значки не
  // грузит: это раскрыло бы сайтам историю и шло бы с куки «Основного» Space.
  // Тон значка на прозрачном фоне: 'light' — почти белый, 'dark' — почти чёрный, '' — цветной
  // или со своим фоном. Сайты отдают значок под свою тему (GitHub в тёмной — белый), и в другой
  // теме Jiyda он сливается с фоном: тогда CSS подкладывает плашку (.fav[data-tone]).
  const tones = new Map();
  function toneOf(img) {
    if (tones.has(img.src)) return tones.get(img.src);
    let tone = '';
    try {
      const c = document.createElement('canvas');
      c.width = c.height = 16;
      const g = c.getContext('2d', { willReadFrequently: true });
      g.drawImage(img, 0, 0, 16, 16);
      const px = g.getImageData(0, 0, 16, 16).data;
      let seen = 0, sum = 0;
      for (let i = 0; i < px.length; i += 4) {
        if (px[i + 3] < 128) continue;
        seen++;
        sum += (0.2126 * px[i] + 0.7152 * px[i + 1] + 0.0722 * px[i + 2]) / 255;
      }
      // Почти непрозрачный значок — со своим фоном, ему плашка не нужна.
      if (seen > 0 && seen < 256 * 0.9) {
        const lum = sum / seen;
        tone = lum > 0.8 ? 'light' : lum < 0.2 ? 'dark' : '';
      }
    } catch (_) { /* битая картинка */ }
    tones.set(img.src, tone);
    return tone;
  }

  // Пометить el тоном значка img (data-tone), когда картинка загрузится; плашку рисует CSS.
  function markTone(img, el) {
    const mark = () => { const tone = toneOf(img); if (tone) el.dataset.tone = tone; };
    if (img.complete && img.naturalWidth) mark(); else img.addEventListener('load', mark, { once: true });
  }

  function fav(p, cls = '') {
    const box = h('span', { class: 'fav ' + cls, attrs: { 'aria-hidden': 'true' } });
    const letter = () => { box.classList.remove('img'); box.textContent = ((p.title || host(p.url) || '?').trim()[0] || '?').toUpperCase(); };
    const src = p.favicon && p.favicon.startsWith('data:image/') ? p.favicon : '';
    if (src) {
      const img = h('img', { src, alt: '', draggable: false });
      img.onerror = () => { img.remove(); letter(); };
      markTone(img, box);
      box.classList.add('img');
      box.append(img);
    } else letter();
    return box;
  }
  const titleOf = (p) => p.title || host(p.url) || t('New page');

  // Сочетание клавиш команды (для подсказок): первое назначенное.
  const keyOf = (cmd) => {
    const k = (state.keys || []).find((x) => x.cmd === cmd);
    return k && k.keys[0] ? k.keys[0].replace(/\+/g, ' ') .replace('Comma', ',').replace('Slash', '/') : '';
  };
  // Подпись с сочетанием для title.
  const withKey = (label, cmd) => (keyOf(cmd) ? `${label} (${keyOf(cmd)})` : label);

  // ---------- API для ядра ----------
  window.jiyda = {
    render(s) {
      state = s;
      applyTheme();
      emit('render', state);
      S.region.update();
    },
    command(cmd, arg) { emit('command:' + cmd, arg); emit('command', cmd, arg); },
    data(kind, payload) { emit('data:' + kind, payload); },
    toast(tst) { emit('toast', tst); },
    preview(id, uri) { emit('preview', id, uri); },
    findResult(r) { emit('findResult', r); },
  };

  return {
    cfg, post, $, $$, h, fill, icon, iconBtn, on, emit, t,
    setDict: (d) => { dict = d; },
    get state() { return state; },
    space, pagesHere, page, activePage, splitOf, colorOf, SPACE_COLORS,
    layers, holdKeyboard, placeAt, show, hide, afterMotion, activeOption,
    host, splitUrl, ago, size, time, fav, markTone, titleOf, keyOf, withKey, nowSec,
    cmd: (k) => post({ type: 'cmd', k }),
    watchPosts: (fn) => { onPost = fn; },
    applyTheme,
  };
})();

// Для отладки и проверок (JIYDA_AUTOMATION: команда js).
window.S = S;
export { S };
