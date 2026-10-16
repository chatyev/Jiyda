// Встраивается в каждую страницу. Ловит Ctrl/средний клик по ссылкам, иконку сайта
// и воспроизведение звука. Горячие клавиши браузера ловит hotkeys.rs, не страница.
(function () {
  if (window.__seyro) return;
  window.__seyro = true;

  const post = (m) => { try { window.ipc.postMessage(JSON.stringify(m)); } catch (_) {} };
  const isTop = window.top === window;

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
