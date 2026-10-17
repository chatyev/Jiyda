// После сбоя: страницы и настройки, сохранённые автосохранением, на месте.
E2E.test('crash/after: pages saved before the crash are restored', async (t) => {
  await t.waitFor(() => t.byUrl('a.html') && t.byUrl('b.html'), 'A и B вернулись');
  t.eq(t.byUrl('b.html').state, 'later', 'B по-прежнему в Later');
});

E2E.test('crash/after: settings saved before the crash are restored', async (t) => {
  const s = await new Promise((resolve) => { S.on('data:settings', resolve); t.post({ type: 'get', what: 'settings' }); });
  t.eq(s.sleep_minutes, 30, 'sleep_minutes');
});
