// История и недавно закрытые.

function history(t) {
  return new Promise((resolve) => {
    const once = (h) => { resolve(h || []); };
    S.on('data:history', once); // обработчик остаётся, но лишние вызовы безвредны
    t.post({ type: 'get', what: 'history' });
  });
}

E2E.test('history: visited pages are recorded', async (t) => {
  await t.open('a.html');
  await t.open('b.html');
  const h = await history(t);
  const urls = h.map((e) => e.url);
  t.ok(urls.includes(t.url('a.html')) && urls.includes(t.url('b.html')), `A и B в истории (${urls.length} записей)`);
  const a = h.find((e) => e.url === t.url('a.html'));
  t.eq(a.title, 'Page A', 'заголовок в истории');
});

E2E.test('history: remove entry and undo', async (t) => {
  await t.open('c.html');
  await t.waitFor(async () => (await history(t)).some((e) => e.url === t.url('c.html')), 'C в истории');
  t.post({ type: 'history_remove', url: t.url('c.html') });
  await t.waitFor(async () => !(await history(t)).some((e) => e.url === t.url('c.html')), 'C удалена из истории');
  t.post({ type: 'history_undo' });
  await t.waitFor(async () => (await history(t)).some((e) => e.url === t.url('c.html')), 'C вернулась после Undo');
});

E2E.test('history: layer lists entries and search filters them', async (t) => {
  await t.open('a.html');
  await t.open('long.html');
  t.cmd('history');
  await t.waitFor(() => t.layerOpen('history'), 'история открыта');
  await t.waitFor(() => t.$$('.history .row').length >= 2, 'строки истории');
  const search = t.$('.history input');
  t.type(search, 'Long');
  await t.waitFor(() => {
    const rows = t.$$('.history .row');
    return rows.length >= 1 && rows.every((r) => /long/i.test(r.textContent));
  }, 'осталась только Long Page');
});

E2E.test('closed: list, restore by index, clear', async (t) => {
  await t.open('a.html');
  const b = await t.open('b.html');
  const c = await t.open('c.html');
  t.post({ type: 'close', id: b.id });
  t.post({ type: 'close', id: c.id });
  await t.waitFor(() => (t.state().closed || []).length === 2, 'две закрытые');
  const entry = t.state().closed.find((x) => x.url === t.url('b.html'));
  t.post({ type: 'restore_closed', index: entry.index });
  await t.waitFor(() => t.byUrl('b.html'), 'B вернулась');
  t.eq(t.state().closed.length, 1, 'в списке осталась одна');
  t.post({ type: 'clear_closed' });
  await t.waitFor(() => t.state().closed.length === 0, 'список пуст');
});

E2E.test('closed: restore all', async (t) => {
  await t.open('a.html');
  const b = await t.open('b.html');
  const c = await t.open('c.html');
  t.post({ type: 'close', id: b.id });
  t.post({ type: 'close', id: c.id });
  await t.waitFor(() => t.state().closed.length === 2, 'две закрытые');
  t.post({ type: 'restore_all_closed' });
  await t.waitFor(() => t.byUrl('b.html') && t.byUrl('c.html'), 'обе вернулись');
});
