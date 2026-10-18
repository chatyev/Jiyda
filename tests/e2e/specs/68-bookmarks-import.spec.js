// Закладки (звёздочка, панель Ctrl+Shift+O, папки, Undo, подсказки) и перенос из
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

E2E.test('bookmarks: star adds the page, second press removes it', async (t) => {
  await clearBookmarks(t);
  const a = await t.open('a.html');
  await t.waitFor(() => !t.$('#addr-star').hidden, 'звёздочка видна у сайта');
  t.ok(!t.state().bookmarked, 'сначала не в закладках');
  t.$('#addr-star').click();
  await t.waitFor(() => t.state().bookmarked, 'страница в закладках');
  t.ok(t.$('#addr-star').classList.contains('on'), 'звёздочка закрашена');
  const items = await bookmarks(t);
  t.eq(items.map((n) => [n.title, n.url]), [[a.title, a.url]], 'закладка с названием и адресом страницы');
  t.cmd('bookmark');
  await t.waitFor(() => !t.state().bookmarked, 'убрана той же командой');
  t.eq((await bookmarks(t)).length, 0, 'закладок нет');
});

E2E.test('bookmarks: panel opens a bookmark as a new page', async (t) => {
  await clearBookmarks(t);
  await t.open('a.html');
  t.cmd('bookmark');
  await t.waitFor(() => t.state().bookmarked, 'A в закладках');
  t.post({ type: 'close', id: t.active().id });
  await t.waitFor(() => !t.state().pages.length, 'страниц нет');
  t.cmd('bookmarks');
  await t.waitFor(() => t.layerOpen('bookmarks'), 'панель закладок');
  const row = await t.waitFor(() => t.$$('.bookmarks .row').find((r) => r.textContent.includes('Page A')), 'строка A');
  row.click();
  await t.waitFor(() => t.active() && t.active().url === t.url('a.html'), 'A открылась');
  t.ok(!t.layerOpen('bookmarks'), 'панель закрылась');
});

E2E.test('bookmarks: folders — create, move into, rename, delete with Undo', async (t) => {
  await clearBookmarks(t);
  await t.open('a.html');
  t.cmd('bookmark');
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
  t.cmd('bookmark');
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
  t.cmd('bookmark');
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
