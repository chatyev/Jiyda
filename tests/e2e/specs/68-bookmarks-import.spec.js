// Закладки (окно Ctrl+D и ☆, панель 🔖 / Ctrl+Shift+O, папки, Undo, подсказки) и перенос из
// Chrome / Edge. «Chrome» и «Edge» — тестовые, из tests/e2e/import (JIYDA_IMPORT_ROOT).

// Закладки, как их хранит ядро.
const bookmarks = (t) => new Promise((resolve) => {
  S.on('data:bookmarks', (b) => resolve(b.items));
  t.post({ type: 'get', what: 'bookmarks' });
});
// Все ссылки (с вложенными папками) и узел по названию.
const links = (items) => items.flatMap((n) => (n.children ? links(n.children) : [n]));
const byTitle = (items, title) => {
  for (const n of items) {
    if (n.title === title) return n;
    const f = n.children && byTitle(n.children, title);
    if (f) return f;
  }
  return null;
};
// Убрать всё, что тест добавил в закладки.
async function clearBookmarks(t) {
  for (const n of await bookmarks(t)) t.post({ type: 'bookmark_remove', id: n.id });
  await t.waitFor(async () => (await bookmarks(t)).length === 0, 'закладки очищены');
}
// Сохранить текущую страницу, как это делает окно Ctrl+D.
const addActive = (t, parent = null) => t.post({ type: 'bookmark_add', url: t.active().url, title: t.active().title, parent });
const dialogOpen = (t) => t.layerOpen('bookmark_page') && !t.$('.bm-dialog').hidden;
const dialogBtn = (t, label) => t.$$('.bm-dialog button').find((b) => b.textContent === S.t(label));
const panelRows = (t) => t.$$('.bm-panel .bm-list .row');

E2E.test('bookmarks: star opens the dialog; Save adds, Remove removes', async (t) => {
  await clearBookmarks(t);
  const a = await t.open('a.html');
  await t.waitFor(() => !t.$('#addr-star').hidden, 'звёздочка видна у сайта');
  t.$('#addr-star').click();
  await t.waitFor(() => dialogOpen(t), 'окно закладки');
  t.eq(t.$('.bm-dialog h2').textContent, S.t('Add bookmark'), 'новая закладка');
  t.eq(t.$('.bm-dialog input').value, a.title, 'название — заголовок страницы');
  t.ok(!t.state().bookmarked, 'до «Сохранить» ничего не сохранено');
  t.type(t.$('.bm-dialog input'), 'My A');
  dialogBtn(t, 'Save').click();
  await t.waitFor(() => t.state().bookmarked, 'страница в закладках');
  t.ok(!t.layerOpen('bookmark_page'), 'окно закрылось');
  t.ok(t.$('#addr-star').classList.contains('on'), 'звёздочка закрашена');
  t.eq((await bookmarks(t)).map((n) => [n.title, n.url]), [['My A', a.url]], 'название из окна, адрес страницы');
  // Ctrl+D на сохранённой странице — изменить или удалить.
  t.cmd('bookmark');
  await t.waitFor(() => dialogOpen(t) && t.$('.bm-dialog h2').textContent === S.t('Edit bookmark'), 'окно изменения');
  dialogBtn(t, 'Remove').click();
  await t.waitFor(() => !t.state().bookmarked, 'удалена');
  t.eq((await bookmarks(t)).length, 0, 'закладок нет');
});

E2E.test('bookmarks: Cancel and Esc save nothing', async (t) => {
  await clearBookmarks(t);
  await t.open('a.html');
  t.cmd('bookmark');
  await t.waitFor(() => dialogOpen(t), 'окно закладки');
  dialogBtn(t, 'Cancel').click();
  await t.waitFor(() => !t.layerOpen('bookmark_page'), 'закрыто «Отмена»');
  t.cmd('bookmark');
  await t.waitFor(() => dialogOpen(t), 'снова открыто');
  t.key(t.$('.bm-dialog input'), 'Escape');
  await t.waitFor(() => !t.layerOpen('bookmark_page'), 'закрыто Esc');
  await t.sleep(200);
  t.eq((await bookmarks(t)).length, 0, 'ничего не сохранено');
});

E2E.test('bookmarks: the same address is never saved twice', async (t) => {
  await clearBookmarks(t);
  await t.open('a.html');
  addActive(t);
  await t.waitFor(() => t.state().bookmarked, 'A в закладках');
  t.post({ type: 'bookmark_folder', title: 'Work' });
  const work = await t.waitFor(async () => byTitle(await bookmarks(t), 'Work'), 'папка Work');
  // Окно для уже сохранённой страницы: новое название и папка — у прежней закладки.
  t.post({ type: 'bookmark_add', url: t.active().url, title: 'A at work', parent: work.id });
  const items = await t.waitFor(async () => { const b = await bookmarks(t); return byTitle(b, 'A at work') && b; }, 'изменена');
  t.eq(links(items).length, 1, 'одна закладка');
  t.eq(byTitle(items, 'Work').children.map((n) => n.title), ['A at work'], 'и она в папке Work');
  t.post({ type: 'bookmark_add', url: 'javascript:alert(1)', title: 'X' });
  await t.sleep(300);
  t.eq(links(await bookmarks(t)).length, 1, 'javascript: не сохраняется');
});

E2E.test('bookmarks: the toolbar button toggles a floating panel', async (t) => {
  await clearBookmarks(t);
  await t.open('a.html');
  const btn = t.$('#bm-btn');
  t.ok(btn && btn.offsetParent, 'кнопка видна на верхней панели');
  t.ok(btn.previousElementSibling.id === 'ask-btn' && btn.nextElementSibling.id === 'menu-btn', 'между ✦ и ⋯');
  btn.click();
  await t.waitFor(() => t.layerOpen('bookmarks') && t.$('.bm-panel').classList.contains('open'), 'панель открыта');
  t.eq(btn.getAttribute('aria-expanded'), 'true', 'кнопка нажата');
  t.ok(!S.layers.full(), 'панель не закрывает окно: страница видна и получает мышь');
  const r = t.$('.bm-panel').getBoundingClientRect();
  t.ok(innerWidth - r.right <= 16 && r.top >= S.cfg.chromeH, 'справа под верхней панелью');
  // Пустая — предлагает сохранить текущую страницу.
  t.ok(t.$('.bm-panel .empty'), 'пустое состояние');
  t.$('.bm-panel .bm-add').click();
  await t.waitFor(() => t.state().bookmarked && panelRows(t).length === 1, '«Добавить текущую страницу»');
  t.eq(t.$('.bm-panel .bm-add .row-title').textContent, S.t('Edit bookmark for this page'), 'строка меняется на «Изменить»');
  btn.click();
  await t.waitFor(() => !t.layerOpen('bookmarks'), 'второе нажатие закрывает');
  t.eq(btn.getAttribute('aria-expanded'), 'false', 'кнопка отжата');
  t.cmd('bookmarks');
  await t.waitFor(() => t.layerOpen('bookmarks'), 'Ctrl+Shift+O открывает');
  t.key(t.$('.bm-panel input'), 'Escape');
  await t.waitFor(() => !t.layerOpen('bookmarks'), 'Esc закрывает');
});

E2E.test('bookmarks: panel opens a bookmark in the current page, Ctrl+click in background', async (t) => {
  await clearBookmarks(t);
  await t.open('a.html');
  addActive(t);
  await t.waitFor(() => t.state().bookmarked, 'A в закладках');
  const b = await t.open('b.html', 'current');
  t.cmd('bookmarks');
  const row = await t.waitFor(() => t.layerOpen('bookmarks') && panelRows(t).find((r) => r.textContent.includes('Page A')), 'строка A');
  row.dispatchEvent(new MouseEvent('click', { bubbles: true, ctrlKey: true }));
  await t.waitFor(() => t.state().pages.length === 2, 'A открылась фоном');
  t.eq(t.active().id, b.id, 'текущая страница не сменилась');
  t.ok(t.layerOpen('bookmarks'), 'панель осталась открытой');
  panelRows(t).find((r) => r.textContent.includes('Page A')).click();
  await t.waitFor(() => t.active() && t.active().id === b.id && t.active().url === t.url('a.html'), 'A открылась в текущей странице');
  t.ok(!t.layerOpen('bookmarks'), 'панель закрылась');
});

E2E.test('bookmarks: folders expand in place; search finds by folder name', async (t) => {
  await clearBookmarks(t);
  await t.open('a.html');
  t.post({ type: 'bookmark_folder', title: 'Work' });
  const work = await t.waitFor(async () => byTitle(await bookmarks(t), 'Work'), 'папка Work');
  addActive(t, work.id);
  await t.waitFor(async () => byTitle(await bookmarks(t), 'Work').children.length === 1, 'A в папке');
  t.cmd('bookmarks');
  await t.waitFor(() => t.layerOpen('bookmarks') && panelRows(t).length === 1 && panelRows(t)[0].classList.contains('bm-folder'), 'видна только папка');
  t.eq(panelRows(t)[0].getAttribute('aria-expanded'), 'false', 'свёрнута');
  panelRows(t)[0].click();
  await t.waitFor(() => panelRows(t).length === 2, 'раскрылась: A внутри');
  t.eq(panelRows(t)[0].getAttribute('aria-expanded'), 'true', 'раскрыта');
  t.key(t.$('.bm-panel input'), 'ArrowLeft');
  await t.waitFor(() => panelRows(t).length === 1, '← свернула');
  t.type(t.$('.bm-panel input'), 'work');
  await t.waitFor(() => panelRows(t).length === 2, 'поиск по папке: папка и ссылка в ней');
  t.ok(panelRows(t).some((r) => r.textContent.includes('Page A')), 'нашлась A');
});

E2E.test('bookmarks: folders — create, move into, rename, delete with Undo', async (t) => {
  await clearBookmarks(t);
  await t.open('a.html');
  addActive(t);
  await t.waitFor(() => t.state().bookmarked, 'A в закладках');
  t.post({ type: 'bookmark_folder', title: 'Work' });
  let items = await t.waitFor(async () => { const b = await bookmarks(t); return byTitle(b, 'Work') && b; }, 'папка Work');
  const work = byTitle(items, 'Work');
  const a = links(items)[0];
  t.post({ type: 'bookmark_move', id: a.id, parent: work.id });
  items = await t.waitFor(async () => { const b = await bookmarks(t); return byTitle(b, 'Work').children.length === 1 && b; }, 'A в папке');
  t.eq(items.length, 1, 'наверху только папка');
  t.post({ type: 'bookmark_edit', id: work.id, title: 'Job' });
  await t.waitFor(async () => byTitle(await bookmarks(t), 'Job'), 'папка переименована');
  t.post({ type: 'bookmark_remove', id: work.id });
  await t.waitFor(() => !t.state().bookmarked, 'вместе с папкой ушла и A');
  t.post({ type: 'bookmark_undo' });
  await t.waitFor(() => t.state().bookmarked, 'Undo вернул папку с A');
  t.eq(byTitle(await bookmarks(t), 'Job').children.map((n) => n.url), [t.url('a.html')], 'A снова в папке');
});

E2E.test('bookmarks: rejects a non-web address on edit', async (t) => {
  await clearBookmarks(t);
  await t.open('a.html');
  addActive(t);
  await t.waitFor(() => t.state().bookmarked, 'A в закладках');
  const a = (await bookmarks(t))[0];
  t.post({ type: 'bookmark_edit', id: a.id, title: 'X', url: 'javascript:alert(1)' });
  await t.sleep(300);
  const after = (await bookmarks(t))[0];
  t.eq([after.title, after.url], [a.title, a.url], 'закладка не изменилась');
});

E2E.test('bookmarks: suggested in the address bar and palette', async (t) => {
  await clearBookmarks(t);
  await t.open('a.html');
  addActive(t);
  await t.waitFor(() => t.state().bookmarked, 'A в закладках');
  t.post({ type: 'close', id: t.active().id });
  await t.waitFor(() => !t.state().pages.length, 'страниц нет');
  S.loadHistory();
  const hit = await t.waitFor(() => S.suggest('Page A').find((r) => r.kind === 'bookmark'), 'подсказка-закладка');
  t.eq(hit.url, t.url('a.html'), 'адрес закладки');
  await clearBookmarks(t);
});

E2E.test('import: New Page offers to import from the browsers found', async (t) => {
  await t.waitFor(() => t.state().importOffer, 'ядро предлагает перенос');
  t.eq(t.state().importOffer.browsers, ['Google Chrome', 'Microsoft Edge'], 'найдены оба браузера');
  await t.waitFor(() => !t.$('#home').hidden && !t.$('.home-import').hidden, 'карточка на New Page');
  t.$('.home-import .btn.primary').click();
  await t.waitFor(() => t.layerOpen('settings'), 'Настройки → Перенос');
});

E2E.test('import: Chrome bookmarks land in their own folder, once', async (t) => {
  await clearBookmarks(t);
  S.layers.open('settings', { section: 'import' });
  const btn = await t.waitFor(() => t.$$('.settings .setting').find((r) => r.textContent.includes('Google Chrome'))?.querySelector('button'), 'строка Chrome');
  btn.click();
  let items = await t.waitFor(async () => { const b = await bookmarks(t); return byTitle(b, 'Google Chrome') && b; }, 'папка Google Chrome', 15000);
  const chrome = byTitle(items, 'Google Chrome');
  t.eq(links([chrome]).map((n) => n.url).sort(),
    ['https://docs.fixture.test/', 'https://other.fixture.test/', 'https://wiki.fixture.test/'], 'все ссылки Chrome');
  t.ok(byTitle(chrome.children, 'Work'), 'папка Work сохранилась');
  t.ok(!t.state().importOffer, 'больше не предлагается');
  // Вторая заметка (после «пароли не переносятся») — итог переноса.
  await t.waitFor(() => t.$$('.settings .notice').length === 2, 'итог в Настройках');
  // Повторный перенос ничего не удваивает.
  t.$$('.settings .setting').find((r) => r.textContent.includes('Google Chrome')).querySelector('button').click();
  await t.sleep(1500);
  items = await bookmarks(t);
  t.eq(links(items).length, 3, 'по-прежнему три закладки');
  t.eq(items.length, 1, 'одна папка Google Chrome');
  await clearBookmarks(t);
});
