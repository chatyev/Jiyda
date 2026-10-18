// Перезапуск, часть 2: всё, что собрал session.before.spec.js, восстановилось.
const in0 = (t) => t.state().pages.filter((p) => p.spaceId === t.state().spaces[0].id);

E2E.test('restart/after: pages of the main space are restored', async (t) => {
  await t.waitFor(() => in0(t).length >= 4, 'страницы основного пространства');
  const urls = in0(t).map((p) => p.url).sort();
  t.eq(urls, ['a.html', 'b.html', 'c.html', 'long.html'].map((u) => t.url(u)).sort(), 'адреса');
  const byUrl = (u) => in0(t).find((p) => p.url === t.url(u));
  t.eq(byUrl('a.html').title, 'Page A', 'заголовок сохранился');
});

E2E.test('restart/after: pin and Later survive', async (t) => {
  const byUrl = (u) => in0(t).find((p) => p.url === t.url(u));
  t.ok(byUrl('a.html').pinned, 'A закреплена');
  t.eq(byUrl('c.html').state, 'later', 'C в Later');
  t.eq(byUrl('b.html').state, 'now', 'B в Now');
});

E2E.test('restart/after: active space and page are restored', async (t) => {
  t.eq(t.state().activeSpace, 0, 'активно основное пространство');
  const active = t.active();
  t.ok(active && active.url === t.url('b.html'), `активна B (сейчас ${active && active.url})`);
  await t.waitFor(() => t.active() && !t.active().loading && t.active().title === 'Page B', 'B загрузилась');
});

E2E.test('restart/after: split pair survives', async (t) => {
  t.eq(t.state().splits.length, 1, 'одна пара');
  const s = t.state().splits[0];
  const urls = [s.left, s.right].map((id) => t.page(id).url).sort();
  t.eq(urls, [t.url('b.html'), t.url('long.html')].sort(), 'B и long');
});

E2E.test('restart/after: second space with its name, color and page', async (t) => {
  t.eq(t.state().spaces.length, 2, 'два пространства');
  const s = t.state().spaces[1];
  t.eq([s.name, s.color], ['Работа', 'green'], 'имя и цвет');
  const pages = t.state().pages.filter((p) => p.spaceId === s.id);
  t.eq(pages.map((p) => p.url), [t.url('index.html')], 'его страница');
});

E2E.test('restart/after: theme setting persists', async (t) => {
  t.eq(t.state().theme, 'light', 'тема');
  t.eq(document.documentElement.dataset.theme, 'light', 'интерфейс в светлой теме');
});

E2E.test('restart/after: history and closed pages persist', async (t) => {
  const h = await new Promise((resolve) => { S.on('data:history', resolve); t.post({ type: 'get', what: 'history' }); });
  t.ok(h.some((e) => e.url === t.url('a.html')), 'A в истории после перезапуска');
});

E2E.test('restart/after: a clean exit is not treated as a crash', async (t) => {
  t.ok(!t.state().recovery, 'нет предложения восстановить');
});
