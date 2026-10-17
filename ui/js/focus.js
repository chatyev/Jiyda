// focus.js — Focus Mode (Ctrl+Shift+F): Edge Reveal выключен, второстепенное скрыто,
// адрес едва заметен, тосты без действий не показываются. На секунду — подсказка.
import { S } from './core.js';

(() => {
  let on = false, timer = 0;
  const overlay = S.h('div', { class: 'focus-overlay', hidden: true, attrs: { role: 'status', 'data-hit': '22' } });
  document.body.append(overlay);

  function toggle() {
    on = !on;
    document.body.classList.toggle('focus', on);
    S.emit('focusmode', on);
    clearTimeout(timer);
    S.fill(overlay, 
      S.h('h2', { text: on ? S.t('Focus Mode') : S.t('Focus Mode off') }),
      on ? S.h('p', { text: S.t('Distractions are hidden. Just you and the content.') }) : null,
      S.h('small', { text: on ? S.t('Press {0} to leave', S.keyOf('focus') || 'Ctrl Shift F') : S.t('Navigation is back') }));
    S.show(overlay);
    timer = setTimeout(() => S.hide(overlay), on ? 1700 : 900);
  }

  S.on('command:focus', toggle);
  S.focusMode = () => on;
})();
