// peek.js — Peek: посмотреть ссылку, не открывая страницу (Shift+клик по ссылке).
// Страница предпросмотра — отдельный WebView под интерфейсом; здесь — затемнение,
// рамка и строка с действиями. Закрыли — в Orbit ничего не добавилось; начали
// работать (перешли по ссылке внутри) или нажали «Open as page» — стала страницей.
import { S } from './core.js';

(() => {
  const layer = S.h('div', { class: 'layer', hidden: true });
  const backdrop = S.h('div', { class: 'backdrop' });
  const head = S.h('div', { class: 'peek-head' });
  const frame = S.h('div', { class: 'peek-frame' });
  layer.append(backdrop, head, frame);
  S.$('#layers').append(layer);
  backdrop.addEventListener('mousedown', () => S.post({ type: 'peek_close' }));
  let rect = null;

  // Под предпросмотром интерфейса нет: дыра в регионе.
  S.region.hole(() => (rect && !layer.hidden ? [rect] : []));
  S.region.full(() => !layer.hidden);

  function draw(p) {
    const [x, y, w, h] = p.rect;
    rect = [x, y, w, h];
    head.style.left = x + 'px'; head.style.top = (y - 40) + 'px'; head.style.width = w + 'px';
    frame.style.left = (x - 1) + 'px'; frame.style.top = y + 'px'; frame.style.width = (w + 2) + 'px'; frame.style.height = (h + 1) + 'px';
    S.fill(head, 
      S.fav({ url: p.url }),
      S.h('span', { class: 'ellipsis', text: p.url, title: p.url }),
      S.h('span', { class: 'chip', text: 'Peek' }),
      S.h('button', { class: 'btn', on: { click: () => S.post({ type: 'peek_open' }) } }, S.icon('external', 'i16'), S.t('Open as page')),
      S.iconBtn('x', S.t('Close (Esc)'), () => S.post({ type: 'peek_close' }), 'sm'));
    head.setAttribute('aria-label', S.t('Link preview'));
  }

  S.on('render', (st) => {
    if (st.peek) {
      draw(st.peek);
      if (layer.hidden) { layer.hidden = false; requestAnimationFrame(() => layer.classList.add('open')); }
    } else if (!layer.hidden) {
      rect = null;
      layer.classList.remove('open');
      layer.hidden = true;
    }
  });
})();
