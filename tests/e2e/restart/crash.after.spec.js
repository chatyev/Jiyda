// После сбоя: страницы и настройки, сохранённые автосохранением, на месте; на экран
// ничего не загружается само — New Page предлагает «Открыть снова» (src/recovery.rs).
E2E.test('crash/after: pages saved before the crash are restored', async (t) => {
  await t.waitFor(() => t.byUrl('a.html') && t.byUrl('b.html'), 'A и B вернулись');
  t.eq(t.byUrl('b.html').state, 'later', 'B по-прежнему в Later');
});

E2E.test('crash/after: settings saved before the crash are restored', async (t) => {
  const s = await new Promise((resolve) => { S.on('data:settings', resolve); t.post({ type: 'get', what: 'settings' }); });
  t.eq(s.sleep_minutes, 30, 'sleep_minutes');
});

E2E.test('crash/after: nothing loads by itself, New Page offers to reopen', async (t) => {
  t.ok(t.state().recovery, 'ядро предлагает восстановление');
  t.eq(t.state().active, null, 'активной страницы нет');
  t.ok(t.state().pages.every((p) => p.asleep), 'ни одна страница не загружена');
  await t.waitFor(() => !t.$('#home').hidden && t.$('.home .hc-continue .btn.primary'), 'карточка на New Page');
});

E2E.test('crash/after: "Reopen pages" opens the page that was on screen', async (t) => {
  t.$('.home .hc-continue .btn.primary').click();
  await t.waitFor(() => t.active() && t.active().url === t.url('a.html'), 'активна A');
  await t.waitFor(() => !t.active().asleep && !t.active().loading && t.active().title === 'Page A', 'A загрузилась');
  t.ok(!t.state().recovery, 'предложение снято');
  await t.waitFor(() => t.$('#home').hidden, 'New Page скрыта');
});
