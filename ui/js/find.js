// find.js — поиск по странице (Ctrl+F): плавающее поле справа сверху, над страницей,
// без сдвига её содержимого. Ищет скрипт в странице (src/find.js), сюда приходит счётчик.
import { S } from './core.js';

(() => {
  const input = S.h('input', { attrs: { spellcheck: 'false', autocomplete: 'off' } });
  const count = S.h('span', { class: 'count' });
  const prev = S.iconBtn('chevron-down', 'Previous', () => run('prev'), 'sm');
  prev.firstChild.style.transform = 'rotate(180deg)';
  const next = S.iconBtn('chevron-down', 'Next', () => run('next'), 'sm');
  const closeBtn = S.iconBtn('x', 'Close', () => S.layers.close('find'), 'sm');
  const bar = S.h('div', { class: 'findbar', hidden: true, attrs: { role: 'search', 'data-hit': '22' } }, input, count, prev, next, closeBtn);
  document.body.append(bar);

  const run = (mode) => S.post({ type: 'find', text: input.value, mode });

  S.layers.register('find', {
    open() {
      input.placeholder = S.t('Find on page');
      input.setAttribute('aria-label', S.t('Find on page'));
      prev.title = S.t('Previous (Shift+Enter)'); next.title = S.t('Next (Enter)'); closeBtn.title = S.t('Close (Esc)');
      bar.hidden = false;
      requestAnimationFrame(() => { input.focus(); input.select(); });
      if (input.value) run('type');
    },
    close() {
      bar.hidden = true;
      bar.classList.remove('miss');
      count.textContent = '';
      S.post({ type: 'find_close' });
    },
  });
  input.addEventListener('input', () => run('type'));
  input.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') { e.preventDefault(); run(e.shiftKey ? 'prev' : 'next'); }
  });
  S.on('findResult', (r) => {
    const q = input.value;
    count.textContent = !q ? '' : r.n > 0 ? S.t('{0} of {1}', r.i + 1, r.n) : S.t('No matches');
    bar.classList.toggle('miss', !!q && r.n === 0);
  });
  S.on('command:find', () => S.layers.open('find'));
  // Ядро закрыло поиск (переход на другую страницу) — прячем поле.
  S.on('render', (st) => { if (!st.find && S.layers.isOpen('find')) { bar.hidden = true; S.layers.close('find'); } });
})();
