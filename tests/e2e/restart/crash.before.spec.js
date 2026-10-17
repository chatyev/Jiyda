// restart: kill
// Сбой: Jiyda убивают без нормального выхода. Сессия пишется на диск с задержкой
// после изменений — то, что успело записаться, должно вернуться (crash.after.spec.js).
E2E.test('crash/before: open pages, then wait for autosave', async (t) => {
  await t.open('a.html');
  const b = await t.open('b.html');
  t.post({ type: 'later', id: b.id });
  await t.waitFor(() => t.page(b.id).state === 'later', 'B в Later');
  t.post({ type: 'setting', key: 'sleep_minutes', value: 30 });
  // Автосохранение — через несколько секунд после изменений (SAVE_DELAY в ядре).
  await t.sleep(6000);
});
