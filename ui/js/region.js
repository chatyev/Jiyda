// region.js — где интерфейс должен получать мышь.
//
// WebView интерфейса прозрачный и лежит поверх страницы; окно обрезает его регионом
// (src/frame.rs). Регион = верхняя панель (её добавляет ядро) + видимые элементы с
// атрибутом data-hit (панели, тосты, край Edge Reveal) либо всё окно, пока открыт
// слой во всё окно (Orbit, палитра, New Page). Дыры — Peek: там страница предпросмотра.
// Значение data-hit — запас в пикселях вокруг элемента (тень).
import { S } from './core.js';

S.region = (() => {
  const fullSources = []; // функции: true — интерфейсу нужно всё окно
  const holeSources = []; // функции: [[x, y, w, h], …]
  let pending = false;
  let last = '';

  function rects() {
    const out = [];
    for (const el of document.querySelectorAll('[data-hit]')) {
      if (el.hidden || el.closest('[hidden]')) continue;
      const r = el.getBoundingClientRect();
      if (r.width < 1 || r.height < 1) continue;
      const pad = Number(el.dataset.hit) || 0;
      out.push([r.left - pad, r.top - pad, r.width + 2 * pad, r.height + 2 * pad].map((v) => Math.round(v * 10) / 10));
    }
    return out;
  }

  function send() {
    pending = false;
    const full = S.layers.full() || fullSources.some((f) => f());
    const msg = { type: 'region', full, rects: full ? [] : rects(), holes: holeSources.flatMap((f) => f() || []) };
    const key = JSON.stringify(msg);
    if (key !== last) {
      last = key;
      S.post(msg);
    }
  }

  function update() {
    if (!pending) {
      pending = true;
      requestAnimationFrame(send);
    }
  }

  new MutationObserver(update).observe(document.body, {
    subtree: true, childList: true, attributes: true, attributeFilter: ['class', 'hidden', 'style', 'data-hit'],
  });
  addEventListener('resize', update);
  document.addEventListener('transitionend', update, true);

  return {
    update,
    full: (fn) => fullSources.push(fn),
    hole: (fn) => holeSources.push(fn),
  };
})();
