// Нагрузка: то, что пользователь делает быстро и подряд. Падение процесса run.ps1 видит сам
// (паника в журнале, сценарии не закончились).

E2E.test('stress: many pages opened back to back without waiting', async (t) => {
  // Регрессия: создание WebView2 крутит свой цикл сообщений, и если в это время истекал
  // срок ожидания цикла событий (автосохранение, снимок для Orbit), tao вызывал
  // обработчик повторно и падал: «either event handler is re-entrant».
  const urls = [];
  for (let i = 0; i < 6; i++) {
    urls.push(t.url(['a.html', 'b.html', 'c.html'][i % 3]) + '?n=' + i);
    urls.push('https://burst' + i + '.jiyda.test/');
  }
  for (const url of urls) {
    t.post({ type: 'navigate', text: url, mode: 'new' });
    t.post({ type: 'setting', key: 'sleep_minutes', value: 60 }); // изменение → срок автосохранения
    await t.sleep(40);
  }
  await t.waitFor(() => t.nowPages().length === urls.length, `открыто ${urls.length} страниц`, 30000);
  // Браузер жив и отвечает.
  t.cmd('palette');
  await t.waitFor(() => t.layerOpen('palette'), 'палитра открывается после нагрузки');
});

E2E.test('stress: rapid close and reopen', async (t) => {
  for (const p of ['a.html', 'b.html', 'c.html']) await t.open(p);
  for (let i = 0; i < 5; i++) {
    t.cmd('close');
    t.cmd('reopen');
  }
  await t.sleep(1500);
  await t.waitFor(() => t.nowPages().length === 3, `снова три страницы (сейчас ${t.nowPages().length})`);
});

E2E.test('stress: switching spaces quickly', async (t) => {
  await t.open('a.html');
  t.post({ type: 'space_new' });
  await t.waitFor(() => t.state().spaces.length === 2, 'второе пространство');
  S.layers.closeAll();
  await t.open('b.html');
  for (let i = 0; i < 10; i++) t.post({ type: 'space', index: i % 2 });
  await t.waitFor(() => t.state().activeSpace === 1, 'последнее переключение — во второе');
  t.eq(t.active() && t.active().url, t.url('b.html'), 'во втором активна B');
});

E2E.test('stress: layers toggled quickly', async (t) => {
  await t.open('a.html');
  for (const name of ['orbit', 'palette', 'history', 'orbit', 'spaces', 'settings', 'palette']) t.cmd(name);
  await t.sleep(800);
  S.layers.closeAll();
  await t.waitFor(() => !S.layers.any(), 'все слои закрыты');
  t.cmd('palette');
  await t.waitFor(() => t.layerOpen('palette'), 'палитра после быстрых переключений');
});
