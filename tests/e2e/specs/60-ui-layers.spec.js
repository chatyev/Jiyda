// Палитра, Orbit, поиск на странице, панель защиты, New Page — то, что видит пользователь.

E2E.test('palette: finds an open page and switches to it', async (t) => {
  const a = await t.open('a.html');
  await t.open('b.html');
  t.cmd('palette');
  await t.waitFor(() => t.layerOpen('palette'), 'палитра');
  const input = t.$('.palette input');
  t.type(input, 'Page A');
  // Первая строка — поиск в сети по набранному (как в адресной строке Chrome),
  // открытая страница — в группе «Страницы» (с сайтом localhost). Стрелками до неё.
  const isPage = (r) => r.textContent.includes('Page A') && r.textContent.includes('localhost');
  await t.waitFor(() => t.$$('#palette-list .row').some(isPage), 'открытая Page A в результатах');
  const i = t.$$('#palette-list .row').findIndex(isPage);
  for (let k = 0; k < i; k++) t.key(input, 'ArrowDown');
  t.key(input, 'Enter');
  await t.waitFor(() => t.state().active === a.id, 'переключились на A');
  await t.waitFor(() => !t.layerOpen('palette'), 'палитра закрылась');
});

E2E.test('palette: typing an address opens it', async (t) => {
  t.cmd('palette');
  await t.waitFor(() => t.layerOpen('palette'), 'палитра');
  const input = t.$('.palette input');
  t.type(input, t.url('c.html'));
  await t.sleep(150);
  t.key(input, 'Enter');
  await t.waitFor(() => t.active() && t.active().url === t.url('c.html') && t.active().title === 'Page C', 'открылась C');
});

E2E.test('palette: commands are listed', async (t) => {
  t.cmd('palette');
  await t.waitFor(() => t.layerOpen('palette'), 'палитра');
  await t.waitFor(() => t.$$('#palette-list .row').length > 3, 'есть команды');
});

E2E.test('orbit: shows open pages and activates on click', async (t) => {
  const a = await t.open('a.html');
  await t.open('b.html');
  await t.open('c.html');
  t.cmd('orbit');
  await t.waitFor(() => t.layerOpen('orbit'), 'Orbit');
  await t.waitFor(() => t.$$('.orbit [role="option"]').filter((o) => /Page [ABC]/.test(o.textContent)).length >= 3, 'три страницы в Orbit');
  const cardA = t.$$('.orbit [role="option"]').find((o) => o.textContent.includes('Page A'));
  cardA.click();
  await t.waitFor(() => t.state().active === a.id && !t.layerOpen('orbit'), 'A активна, Orbit закрыт');
});

E2E.test('orbit: search filters pages', async (t) => {
  await t.open('a.html');
  await t.open('long.html');
  t.cmd('orbit');
  await t.waitFor(() => t.layerOpen('orbit'), 'Orbit');
  t.type(t.$('.orbit input'), 'Long');
  await t.waitFor(() => {
    const opts = t.$$('.orbit [role="option"]').filter((o) => /Page A|Long Page/.test(o.textContent));
    return opts.length === 1 && opts[0].textContent.includes('Long Page');
  }, 'осталась только Long Page');
});

E2E.test('find: counts matches on the page', async (t) => {
  await t.open('long.html');
  t.cmd('find');
  await t.waitFor(() => t.layerOpen('find') && !t.$('.findbar').hidden, 'поиск открыт');
  t.type(t.$('.findbar input'), 'needle');
  await t.waitFor(() => /5/.test(t.$('.findbar .count').textContent), `5 совпадений (сейчас «${t.$('.findbar .count').textContent}»)`);
  t.type(t.$('.findbar input'), 'nothing-like-this');
  await t.waitFor(() => t.$('.findbar').classList.contains('miss'), 'нет совпадений');
});

E2E.test('find: closes when switching to another page', async (t) => {
  const a = await t.open('a.html');
  await t.open('long.html');
  t.cmd('find');
  await t.waitFor(() => t.layerOpen('find'), 'поиск открыт');
  t.post({ type: 'activate', id: a.id });
  await t.waitFor(() => !t.layerOpen('find') && !t.state().find, 'поиск закрылся');
});

E2E.test('find: stays open when the same page navigates', async (t) => {
  // Как в Chrome: поиск относится к странице, а не к адресу.
  await t.open('long.html');
  t.cmd('find');
  await t.waitFor(() => t.layerOpen('find'), 'поиск открыт');
  await t.open('a.html', 'current');
  await t.sleep(300);
  t.ok(t.layerOpen('find') && t.state().find, 'поиск остался открытым');
});

E2E.test('security: panel opens for a page', async (t) => {
  await t.open('a.html');
  t.$('#privacy').click();
  await t.waitFor(() => t.layerOpen('security'), 'панель защиты');
  await t.waitFor(() => t.$('.pop.security') && t.$('.pop.security').textContent.includes('localhost'), 'в панели — сайт');
});

E2E.test('home: New Page shows search and opens typed address', async (t) => {
  await t.waitFor(() => t.$('.home') && !t.$('.home').hidden, 'New Page видна');
  const input = t.$('.home input');
  t.ok(input, 'поле поиска');
  t.type(input, t.url('a.html'));
  t.key(input, 'Enter');
  await t.waitFor(() => t.active() && t.active().url === t.url('a.html'), 'открылась A');
});

E2E.test('peek: Shift+click opens preview, close keeps page count', async (t) => {
  await t.open('a.html');
  await t.evalPage('document.getElementById("to-b").dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true, shiftKey: true, button: 0 }))');
  await t.waitFor(() => t.state().peek && t.state().peek.url === t.url('b.html'), 'Peek с B');
  t.eq(t.nowPages().length, 1, 'страница не добавилась');
  t.post({ type: 'peek_close' });
  await t.waitFor(() => !t.state().peek, 'Peek закрыт');
  t.eq(t.nowPages().length, 1, 'страниц по-прежнему одна');
});

E2E.test('peek: open as page', async (t) => {
  await t.open('a.html');
  await t.evalPage('document.getElementById("to-b").dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true, shiftKey: true, button: 0 }))');
  await t.waitFor(() => t.state().peek, 'Peek открыт');
  t.post({ type: 'peek_open' });
  await t.waitFor(() => !t.state().peek && t.byUrl('b.html'), 'B стала страницей');
});

E2E.test('pages: Ctrl+click opens link in background', async (t) => {
  const a = await t.open('a.html');
  await t.evalPage('document.getElementById("to-b").dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true, ctrlKey: true, button: 0 }))');
  await t.waitFor(() => t.byUrl('b.html'), 'B открылась');
  t.eq(t.state().active, a.id, 'A осталась активной');
});
