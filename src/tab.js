// Встраивается в каждую страницу. Ловит Ctrl/средний клик и Shift+клик (Peek) по ссылкам,
// воспроизведение звука и показывает панель над выделенным текстом.
// Горячие клавиши браузера ловит hotkeys.rs, не страница.
(function () {
  if (window.__jiyda) return;
  window.__jiyda = true;

  const cfg = window.__JIYDA_PAGE || { selection: true, lang: 'en' };
  const post = (m) => { try { window.ipc.postMessage(JSON.stringify(m)); } catch (_) {} };
  const isTop = window.top === window;

  function linkFrom(e) {
    let n = e.target;
    while (n && n.tagName !== 'A') n = n.parentElement;
    return n && /^https?:/i.test(n.href || '') ? n : null;
  }
  function take(e, msg) {
    e.preventDefault();
    e.stopImmediatePropagation();
    post(msg);
  }
  // Ctrl+клик и клик колёсиком — открыть в фоне; Shift+клик — Peek (предпросмотр).
  document.addEventListener('click', (e) => {
    if (e.button !== 0) return;
    const a = linkFrom(e);
    if (!a) return;
    if (e.ctrlKey || e.metaKey) take(e, { t: 'open_bg', u: a.href });
    else if (e.shiftKey && !e.altKey) take(e, { t: 'peek', u: a.href });
  }, true);
  document.addEventListener('auxclick', (e) => { const a = e.button === 1 && linkFrom(e); if (a) take(e, { t: 'open_bg', u: a.href }); }, true);
  document.addEventListener('mousedown', (e) => { if (e.button === 1 && linkFrom(e)) e.preventDefault(); }, true);

  if (!isTop) return;

  // Играет ли звук/видео — такие страницы не усыпляем.
  let playing = false;
  const upd = () => {
    const now = [...document.querySelectorAll('video,audio')].some((m) => !m.paused && !m.muted && !m.ended);
    if (now !== playing) { playing = now; post({ t: 'media', on: now }); }
  };
  ['play', 'pause', 'ended', 'volumechange', 'emptied'].forEach((ev) => document.addEventListener(ev, upd, true));

  // ---------- Панель над выделенным текстом ----------
  // В закрытом shadow DOM: стили страницы её не трогают, а страница её не видит.
  const L = {
    ru: ['Копировать', 'Найти', 'Объяснить', 'Перевести'],
    uz: ['Nusxa olish', 'Qidirish', 'Tushuntirish', 'Tarjima qilish'],
  }[cfg.lang] || ['Copy', 'Search', 'Explain', 'Translate'];
  const ACTIONS = [
    ['copy', L[0]],
    ['search', L[1]],
    ['explain', L[2]],
    ['translate', L[3]],
    ['ask', 'Ask Jiyda'],
  ];
  let enabled = cfg.selection !== false;
  let host = null, root = null, bar = null, current = '';
  window.__jiydaSel = (on) => { enabled = !!on; if (!enabled) hide(); };

  function build() {
    host = document.createElement('jiyda-selection');
    host.style.cssText = 'all:initial;position:fixed;z-index:2147483647;left:0;top:0;';
    root = host.attachShadow({ mode: 'closed' });
    // Цвета — те же, что в ui/styles.css (--s2, --text, --line-2, --hover, --accent, --shadow-pop).
    // Тема страницы совпадает с темой Jiyda (ядро задаёт её WebView), и при смене панель перекрашивается сама.
    root.innerHTML = `<style>
      :host{--bg:#ffffff;--fg:#10151C;--line:rgba(15,23,42,.12);--hover:rgba(15,23,42,.06);--accent:#3B6E2C;
        --shadow:0 6px 16px rgba(15,23,42,.14),0 1px 3px rgba(15,23,42,.10)}
      @media (prefers-color-scheme: dark){:host{--bg:#151B23;--fg:#F4F6F8;--line:rgba(255,255,255,.10);
        --hover:rgba(255,255,255,.07);--accent:#9CCB7B;--shadow:0 6px 16px rgba(0,0,0,.34),0 1px 3px rgba(0,0,0,.25)}}
      .bar{display:flex;gap:2px;padding:4px;border-radius:10px;font:500 12px/1 "Segoe UI Variable Text","Segoe UI",system-ui,sans-serif;
        background:var(--bg);color:var(--fg);border:1px solid var(--line);box-shadow:var(--shadow);
        opacity:0;transform:translateY(4px);transition:opacity .12s ease,transform .12s ease}
      .bar.on{opacity:1;transform:none}
      button{all:unset;cursor:default;padding:7px 9px;border-radius:7px;white-space:nowrap}
      button:hover,button:focus-visible{background:var(--hover)}
      button:focus-visible{outline:2px solid var(--accent);outline-offset:-2px}
      .ai{color:var(--accent)}
      @media (prefers-reduced-motion: reduce){.bar{transition:none}}
    </style><div class="bar" role="toolbar" aria-label="Selection"></div>`;
    bar = root.querySelector('.bar');
    for (const [a, label] of ACTIONS) {
      const b = document.createElement('button');
      b.textContent = label;
      if (a === 'ask') b.className = 'ai';
      b.addEventListener('mousedown', (e) => e.preventDefault());
      b.addEventListener('click', (e) => { e.preventDefault(); e.stopPropagation(); act(a); });
      bar.appendChild(b);
    }
    document.documentElement.appendChild(host);
  }

  function act(a) {
    const text = current;
    hide();
    if (a === 'copy') {
      try { navigator.clipboard.writeText(text); } catch (_) { document.execCommand('copy'); }
      return;
    }
    post({ t: 'sel', a, s: text });
  }

  function hide() {
    if (bar) bar.classList.remove('on');
    if (host) host.style.display = 'none';
  }

  function inEditable(sel) {
    const n = sel.anchorNode && (sel.anchorNode.nodeType === 1 ? sel.anchorNode : sel.anchorNode.parentElement);
    return !!(n && n.closest && n.closest('input, textarea, [contenteditable=""], [contenteditable="true"]'));
  }

  function show() {
    if (!enabled) return;
    const sel = window.getSelection();
    const text = sel ? sel.toString().trim() : '';
    if (!text || text.length < 2 || sel.rangeCount === 0 || inEditable(sel)) { hide(); return; }
    const r = sel.getRangeAt(0).getBoundingClientRect();
    if (!r || (r.width === 0 && r.height === 0)) { hide(); return; }
    if (!host) build();
    if (!host.isConnected) document.documentElement.appendChild(host);
    current = text.slice(0, 20000);
    host.style.display = 'block';
    const bw = bar.offsetWidth || 320, bh = bar.offsetHeight || 34;
    let x = Math.min(Math.max(8, r.left + r.width / 2 - bw / 2), innerWidth - bw - 8);
    let y = r.top - bh - 8;
    if (y < 8) y = r.bottom + 8;
    host.style.left = x + 'px';
    host.style.top = y + 'px';
    requestAnimationFrame(() => bar.classList.add('on'));
  }

  document.addEventListener('mouseup', (e) => {
    if (e.button !== 0 || (host && e.composedPath().includes(host))) return;
    setTimeout(show, 10);
  }, true);
  document.addEventListener('keyup', (e) => { if (e.shiftKey && e.key.startsWith('Arrow')) setTimeout(show, 10); }, true);
  document.addEventListener('mousedown', (e) => { if (!host || !e.composedPath().includes(host)) hide(); }, true);
  document.addEventListener('keydown', (e) => { if (e.key === 'Escape') hide(); }, true);
  // Esc, который страница не использовала сама, закрывает Peek (браузер слушает его только у Peek).
  window.addEventListener('keydown', (e) => { if (e.key === 'Escape' && !e.defaultPrevented) post({ t: 'esc' }); });
  window.addEventListener('scroll', hide, true);
  window.addEventListener('blur', hide);
})();
