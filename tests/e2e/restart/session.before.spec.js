// Перезапуск, часть 1: собрать типичную сессию. Проверяет её session.after.spec.js
// уже в новом процессе Jiyda с тем же профилем.
E2E.test('restart/before: build a session', async (t) => {
  const a = await t.open('a.html');
  const b = await t.open('b.html');
  const c = await t.open('c.html');
  t.post({ type: 'pin', id: a.id, on: true });
  t.post({ type: 'later', id: c.id });
  await t.waitFor(() => t.page(a.id).pinned && t.page(c.id).state === 'later', 'A закреплена, C в Later');
  // Пара A + long.
  const l = await t.open('long.html');
  t.post({ type: 'split', id: b.id });
  await t.waitFor(() => t.state().splits.length === 1, 'пара B + long');
  t.ok(l.id, 'long открыта');

  // Второе пространство со своей страницей и именем.
  t.post({ type: 'space_new' });
  await t.waitFor(() => t.state().spaces.length === 2 && t.state().activeSpace === 1, 'второе пространство');
  S.layers.closeAll();
  t.post({ type: 'space_rename', index: 1, name: 'Работа' });
  t.post({ type: 'space_color', index: 1, color: 'green' });
  await t.open('index.html');
  await t.waitFor(() => t.state().spaces[1].name === 'Работа', 'имя');

  // Вернуться в основное и сделать активной B; тема — светлая.
  t.post({ type: 'space', index: 0 });
  await t.waitFor(() => t.state().activeSpace === 0, 'основное');
  t.post({ type: 'activate', id: b.id });
  await t.waitFor(() => t.state().active === b.id, 'активна B');
  t.post({ type: 'setting', key: 'theme', value: 'light' });
  await t.waitFor(() => t.state().theme === 'light', 'светлая тема');
  // Запись сессии на диск идёт с задержкой — даём ей случиться до закрытия
  // (при закрытии ядро тоже сохраняет, но так проверяем оба пути).
  await t.sleep(1500);
});
