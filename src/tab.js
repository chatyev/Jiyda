// Встраивается в каждую страницу. Ловит горячие клавиши браузера,
// Ctrl/средний клик по ссылкам, иконку сайта и воспроизведение звука.
(function () {
  if (window.__drift) return;
  window.__drift = true;

  const post = (m) => { try { window.ipc.postMessage(JSON.stringify(m)); } catch (_) {} };
  const isTop = window.top === window;

  window.addEventListener('keydown', (e) => {
    const c = e.ctrlKey || e.metaKey, s = e.shiftKey, a = e.altKey;
    if (!c || a) return;
    const k = e.key, kl = k.length === 1 ? k.toLowerCase() : k;
    // На русской раскладке e.key будет кириллицей, поэтому смотрим ещё и e.code.
    const code = e.code;
    let cmd = null;
    if ((kl === 't' || code === 'KeyT') && s) cmd = 'reopen';
    else if (kl === 't' || code === 'KeyT') cmd = 'new_tab';
    else if (kl === 'l' || code === 'KeyL') cmd = 'edit_url';
    else if (kl === 'w' || code === 'KeyW') cmd = 'close';
    else if (k === 'Tab') cmd = s ? 'prev' : 'next';
    else if ((kl === 's' || code === 'KeyS') && !s) cmd = 'toggle_sidebar';
    else if ((kl === 'd' || code === 'KeyD') && !s) cmd = 'pin';
    else if ((kl === 'u' || code === 'KeyU') && s) cmd = 'copy_url';
    else if ((kl === 'f' || code === 'KeyF') && !s) cmd = 'find';
    else if ((kl === 'j' || code === 'KeyJ') && !s) cmd = 'downloads';
    else if (/^Digit[1-9]$/.test(code) && !s) cmd = 'goto:' + code.slice(5);
    if (!cmd) return;
    e.preventDefault();
    e.stopImmediatePropagation();
    if (cmd === 'copy_url') { copy(window.top.location.href); post({ t: 'key', k: 'copied' }); return; }
    post({ t: 'key', k: cmd });
  }, true);

  function copy(text) {
    if (navigator.clipboard && window.isSecureContext) {
      navigator.clipboard.writeText(text).catch(() => fallbackCopy(text));
    } else fallbackCopy(text);
  }
  function fallbackCopy(text) {
    const ta = document.createElement('textarea');
    ta.value = text; ta.style.position = 'fixed'; ta.style.opacity = '0';
    document.documentElement.appendChild(ta); ta.select();
    try { document.execCommand('copy'); } catch (_) {}
    ta.remove();
  }

  // Ctrl+клик и клик колёсиком по ссылке — открыть в фоновой вкладке.
  function linkFrom(e) {
    let n = e.target;
    while (n && n.tagName !== 'A') n = n.parentElement;
    return n && /^https?:/i.test(n.href || '') ? n : null;
  }
  function bg(e) {
    const a = linkFrom(e);
    if (!a) return;
    e.preventDefault();
    e.stopImmediatePropagation();
    post({ t: 'open_bg', u: a.href });
  }
  document.addEventListener('click', (e) => { if (e.button === 0 && (e.ctrlKey || e.metaKey)) bg(e); }, true);
  document.addEventListener('auxclick', (e) => { if (e.button === 1) bg(e); }, true);
  document.addEventListener('mousedown', (e) => { if (e.button === 1 && linkFrom(e)) e.preventDefault(); }, true);

  if (!isTop) return;

  // Иконка сайта.
  const sendIcon = () => {
    const l = document.querySelector('link[rel~="icon"], link[rel="shortcut icon"], link[rel="apple-touch-icon"]');
    const u = l && l.href ? l.href : (/^https?:/.test(location.protocol) ? location.origin + '/favicon.ico' : '');
    post({ t: 'icon', u });
  };
  document.addEventListener('DOMContentLoaded', sendIcon);
  window.addEventListener('load', sendIcon);

  // Играет ли звук/видео — такие вкладки не усыпляем.
  let playing = false;
  const upd = () => {
    const now = [...document.querySelectorAll('video,audio')].some((m) => !m.paused && !m.muted && !m.ended);
    if (now !== playing) { playing = now; post({ t: 'media', on: now }); }
  };
  ['play', 'pause', 'ended', 'volumechange', 'emptied'].forEach((ev) => document.addEventListener(ev, upd, true));
})();
