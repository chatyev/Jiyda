// Страницы: открыть, переключить, закрыть с Undo, перейти по адресу, назад/вперёд.

E2E.test('pages: open from address bar becomes active with title', async (t) => {
  const a = await t.open('a.html');
  t.eq(a.title, 'Page A', 'заголовок');
  t.eq(t.state().active, a.id, 'активная');
  t.eq(t.nowPages().length, 1, 'страниц в Now');
});

E2E.test('pages: second page, then activate the first', async (t) => {
  const a = await t.open('a.html');
  const b = await t.open('b.html');
  t.ok(a.id !== b.id, 'разные страницы');
  t.eq(t.nowPages().length, 2, 'страниц в Now');
  t.post({ type: 'activate', id: a.id });
  await t.waitFor(() => t.state().active === a.id, 'активна A');
});

E2E.test('pages: navigate current page replaces its address', async (t) => {
  const a = await t.open('a.html');
  const b = await t.open('b.html', 'current');
  t.eq(b.id, a.id, 'та же страница');
  t.eq(t.nowPages().length, 1, 'новая не появилась');
});

E2E.test('pages: page content is the right one', async (t) => {
  await t.open('b.html');
  t.eq(await t.evalPage('document.getElementById("marker").textContent'), 'This is page B.', 'текст на странице');
});

E2E.test('pages: close active switches to a neighbour', async (t) => {
  const a = await t.open('a.html');
  const b = await t.open('b.html');
  const c = await t.open('c.html');
  t.post({ type: 'activate', id: b.id });
  await t.waitFor(() => t.state().active === b.id, 'активна B');
  t.post({ type: 'close', id: b.id });
  await t.waitFor(() => !t.page(b.id), 'B закрыта');
  t.ok([a.id, c.id].includes(t.state().active), `активна соседняя (active=${t.state().active})`);
});

E2E.test('pages: reopen brings back the closed page', async (t) => {
  await t.open('a.html');
  const b = await t.open('b.html');
  t.post({ type: 'close', id: b.id });
  await t.waitFor(() => !t.page(b.id), 'B закрыта');
  t.ok((t.state().closed || []).some((c) => c.url === t.url('b.html')), 'B в недавно закрытых');
  t.cmd('reopen');
  await t.waitFor(() => t.byUrl('b.html'), 'B вернулась');
  await t.waitFor(() => t.active() && t.active().url === t.url('b.html'), 'и стала активной');
});

E2E.test('pages: closing the last page shows New Page', async (t) => {
  const a = await t.open('a.html');
  t.post({ type: 'close', id: a.id });
  await t.waitFor(() => t.state().pages.length === 0, 'страниц нет');
  t.eq(t.state().active, null, 'активной нет');
  await t.waitFor(() => t.$('.home') && !t.$('.home').hidden, 'New Page видна');
});

E2E.test('pages: back and forward', async (t) => {
  await t.open('a.html');
  await t.evalPage('document.getElementById("to-b").click()');
  await t.waitFor(() => t.active().url === t.url('b.html') && t.active().title === 'Page B', 'перешли на B');
  t.cmd('back');
  await t.waitFor(() => t.active().url === t.url('a.html'), 'назад на A');
  t.cmd('forward');
  await t.waitFor(() => t.active().url === t.url('b.html'), 'вперёд на B');
});

E2E.test('pages: next/prev cycle through pages', async (t) => {
  const a = await t.open('a.html');
  const b = await t.open('b.html');
  t.cmd('next');
  await t.waitFor(() => t.state().active !== b.id, 'next ушёл с B');
  t.cmd('prev');
  await t.waitFor(() => t.state().active === b.id, 'prev вернул B');
  t.ok(a.id, 'A есть');
});

E2E.test('pages: duplicate opens a copy', async (t) => {
  const a = await t.open('a.html');
  t.post({ type: 'duplicate', id: a.id });
  await t.waitFor(() => t.nowPages().filter((p) => p.url === a.url).length === 2, 'две страницы A');
});
