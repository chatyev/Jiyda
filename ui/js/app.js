// app.js — запуск интерфейса: общие правила для всего окна и сигнал ядру «готов».
import { S } from './core.js';

(() => {
  // Свои меню вместо меню WebView (кроме полей ввода — там нужны «Копировать/Вставить»).
  addEventListener('contextmenu', (e) => {
    if (!e.target.closest('input, textarea')) e.preventDefault();
  });

  // Перетаскивание в интерфейс: ссылку — открыть новой страницей; файлы и прочее —
  // игнорировать (иначе WebView интерфейса ушёл бы на файл).
  addEventListener('dragover', (e) => { if (!e.defaultPrevented) e.preventDefault(); });
  addEventListener('drop', (e) => {
    if (e.defaultPrevented) return;
    e.preventDefault();
    const url = (e.dataTransfer.getData('text/uri-list') || e.dataTransfer.getData('text/plain') || '').split('\n')[0].trim();
    if (/^https?:\/\//i.test(url)) S.post({ type: 'navigate', text: url, mode: 'new' });
  });

  // Средняя кнопка мыши не должна запускать автопрокрутку интерфейса.
  addEventListener('mousedown', (e) => { if (e.button === 1) e.preventDefault(); });

  S.post({ type: 'ready' });
})();
