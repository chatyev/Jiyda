// toast.js — короткие сообщения внизу окна. Вместо подтверждений — «Отменить» (Undo):
// «Page closed · Undo», «Moved to Later · Undo», «Removed from history · Undo».
import { S } from './core.js';

(() => {
  const box = S.$('#toasts');
  let current = null, timer = 0;

  function dismiss(el) {
    if (!el) return;
    el.classList.remove('open');
    S.afterMotion(() => { el.remove(); S.region.update(); });
    if (current === el) current = null;
  }

  function show(tst) {
    // В Focus Mode браузер не отвлекает: остаются только сообщения с действием.
    if (document.body.classList.contains('focus') && !tst.action) return;
    dismiss(current);
    clearTimeout(timer);
    const el = S.h('div', { class: 'toast' + (tst.action ? '' : ' no-action'), attrs: { 'data-hit': '22' } },
      S.h('span', { class: 'toast-text', text: tst.text }));
    if (tst.action) {
      el.append(S.h('button', {
        class: 'btn', text: tst.action.label,
        on: { click: () => { S.post(tst.action.msg); dismiss(el); } },
      }));
    }
    el.addEventListener('mouseenter', () => clearTimeout(timer));
    el.addEventListener('mouseleave', () => { timer = setTimeout(() => dismiss(el), 2500); });
    box.append(el);
    current = el;
    requestAnimationFrame(() => el.classList.add('open'));
    timer = setTimeout(() => dismiss(el), tst.action ? 6000 : 3200);
  }

  S.on('toast', show);
  S.toast = (text, action) => show({ text, action });
})();
