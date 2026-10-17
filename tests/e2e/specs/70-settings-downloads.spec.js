// Настройки (применение и проверка значений) и загрузки.

function settings(t) {
  return new Promise((resolve) => {
    S.on('data:settings', (s) => resolve(s));
    t.post({ type: 'get', what: 'settings' });
  });
}

E2E.test('settings: valid values are applied', async (t) => {
  t.post({ type: 'setting', key: 'sleep_minutes', value: 30 });
  t.post({ type: 'setting', key: 'archive_hours', value: 24 });
  t.post({ type: 'setting', key: 'search_url', value: 'https://duckduckgo.com/?q=%s' });
  await t.sleep(300);
  const s = await settings(t);
  t.eq(s.sleep_minutes, 30, 'sleep_minutes');
  t.eq(s.archive_hours, 24, 'archive_hours');
  t.eq(s.search_url, 'https://duckduckgo.com/?q=%s', 'search_url');
});

E2E.test('settings: invalid search address is rejected', async (t) => {
  const before = (await settings(t)).search_url;
  t.post({ type: 'setting', key: 'search_url', value: 'http://insecure.example/?q=%s' });
  await t.sleep(300);
  t.eq((await settings(t)).search_url, before, 'адрес без https не принят');
  t.post({ type: 'setting', key: 'search_url', value: 'https://example.com/search' });
  await t.sleep(300);
  t.eq((await settings(t)).search_url, before, 'адрес без %s не принят');
});

E2E.test('settings: theme setting updates state', async (t) => {
  t.post({ type: 'setting', key: 'theme', value: 'light' });
  await t.waitFor(() => t.state().theme === 'light' && document.documentElement.dataset.theme === 'light', 'светлая');
  t.post({ type: 'setting', key: 'theme', value: 'dark' });
  await t.waitFor(() => t.state().theme === 'dark' && document.documentElement.dataset.theme === 'dark', 'тёмная');
});

E2E.test('settings: search from address bar uses the search template', async (t) => {
  // Шаблон обязан быть https (см. «invalid search address»); домен .test не существует —
  // страница не загрузится, но адрес проверить можно.
  t.post({ type: 'setting', key: 'search_url', value: 'https://search.jiyda.test/?q=%s' });
  await t.waitFor(async () => (await settings(t)).search_url === 'https://search.jiyda.test/?q=%s', 'шаблон принят');
  t.post({ type: 'navigate', text: 'hello world', mode: 'new' });
  await t.waitFor(() => t.active() && t.active().url.startsWith('https://search.jiyda.test/?q=hello'), 'поиск по шаблону');
  t.post({ type: 'setting', key: 'search_url', value: 'https://www.google.com/search?q=%s' });
});

E2E.test('settings: selection toolbar and edge reveal toggles reach state', async (t) => {
  t.post({ type: 'setting', key: 'selection_toolbar', value: false });
  t.post({ type: 'setting', key: 'edge_reveal', value: false });
  await t.waitFor(() => t.state().prefs.selectionToolbar === false && t.state().prefs.edgeReveal === false, 'выключены');
  t.post({ type: 'setting', key: 'selection_toolbar', value: true });
  t.post({ type: 'setting', key: 'edge_reveal', value: true });
  await t.waitFor(() => t.state().prefs.selectionToolbar && t.state().prefs.edgeReveal, 'включены');
});

E2E.test('settings: wrong value type is ignored', async (t) => {
  const before = (await settings(t)).sleep_minutes;
  t.post({ type: 'setting', key: 'sleep_minutes', value: 'много' });
  await t.sleep(300);
  t.eq((await settings(t)).sleep_minutes, before, 'значение не изменилось');
});

E2E.test('downloads: file is downloaded and listed as done', async (t) => {
  let list = [];
  S.on('data:downloads', (d) => { list = d || []; });
  await t.open('download.html');
  await t.evalPage('document.getElementById("dl").click()');
  await t.waitFor(() => list.some((d) => d.url === t.url('file.txt') && d.state === 'done'), 'загрузка завершена', 20000);
  const d = list.find((x) => x.url === t.url('file.txt'));
  t.ok(/file.*\.txt$/i.test(d.name), `имя файла (${d.name})`);
  t.cmd('downloads');
  await t.waitFor(() => t.layerOpen('downloads'), 'список загрузок');
});
