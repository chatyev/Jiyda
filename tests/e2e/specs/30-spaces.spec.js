// Пространства: создать, переименовать, сменить цвет, переключиться, перенести страницу, удалить.

async function newSpace(t) {
  const n = t.state().spaces.length;
  t.post({ type: 'space_new' });
  await t.waitFor(() => t.state().spaces.length === n + 1 && t.state().activeSpace === n, 'новое пространство активно');
  S.layers.closeAll(); // после создания открывается переключатель с полем имени
  return n;
}

E2E.test('spaces: new space becomes active and empty', async (t) => {
  await t.open('a.html');
  const idx = await newSpace(t);
  t.eq(t.pages().length, 0, 'в новом пространстве страниц нет');
  t.eq(t.state().spaces[idx].main, false, 'не основное');
});

E2E.test('spaces: pages belong to their space', async (t) => {
  const a = await t.open('a.html');
  await newSpace(t);
  const b = await t.open('b.html');
  t.eq(t.pages().map((p) => p.id), [b.id], 'во втором — только B');
  t.post({ type: 'space', index: 0 });
  await t.waitFor(() => t.state().activeSpace === 0, 'вернулись в основное');
  t.eq(t.pages().map((p) => p.id), [a.id], 'в основном — только A');
  t.eq(t.state().active, a.id, 'активна A');
});

E2E.test('spaces: rename and recolor', async (t) => {
  const idx = await newSpace(t);
  t.post({ type: 'space_rename', index: idx, name: '  Работа  ' });
  await t.waitFor(() => t.state().spaces[idx].name === 'Работа', 'имя без лишних пробелов');
  t.post({ type: 'space_rename', index: idx, name: '   ' });
  await t.sleep(300);
  t.eq(t.state().spaces[idx].name, 'Работа', 'пустое имя не принимается');
  t.post({ type: 'space_color', index: idx, color: 'green' });
  await t.waitFor(() => t.state().spaces[idx].color === 'green', 'цвет');
});

E2E.test('spaces: move page to another space', async (t) => {
  await t.open('a.html');
  const b = await t.open('b.html');
  const idx = await newSpace(t);
  t.post({ type: 'space', index: 0 });
  await t.waitFor(() => t.state().activeSpace === 0, 'основное');
  t.post({ type: 'move_to_space', id: b.id, space: idx });
  await t.waitFor(() => !t.pages().some((p) => p.url === t.url('b.html')), 'B ушла из основного');
  t.post({ type: 'space', index: idx });
  await t.waitFor(() => t.state().activeSpace === idx, 'второе');
  await t.waitFor(() => t.pages().some((p) => p.url === t.url('b.html')), 'B во втором');
});

E2E.test('spaces: delete removes the space and its pages', async (t) => {
  const idx = await newSpace(t);
  await t.open('c.html');
  t.post({ type: 'space_delete', index: idx });
  await t.waitFor(() => t.state().spaces.length === 1 && t.state().activeSpace === 0, 'осталось основное');
  t.ok(!t.state().pages.some((p) => p.url === t.url('c.html')), 'страницы удалённого пространства нет');
});

E2E.test('spaces: deleted spaces do not block creating new ones', async (t) => {
  // Регрессия: после 8 «создать — удалить» за сеанс новое пространство не создавалось.
  for (let i = 0; i < 10; i++) {
    const idx = await newSpace(t);
    t.post({ type: 'space_delete', index: idx });
    await t.waitFor(() => t.state().spaces.length === 1, `удалено (${i + 1})`);
  }
  await newSpace(t);
  t.eq(t.state().spaces.length, 2, 'одиннадцатое создалось');
});

E2E.test('spaces: main space cannot be deleted', async (t) => {
  t.post({ type: 'space_delete', index: 0 });
  await t.sleep(400);
  t.eq(t.state().spaces.length, 1, 'основное на месте');
});

E2E.test('spaces: switch by command space:N', async (t) => {
  await newSpace(t);
  t.cmd('space:1');
  await t.waitFor(() => t.state().activeSpace === 0, 'space:1 — основное');
  t.cmd('space:2');
  await t.waitFor(() => t.state().activeSpace === 1, 'space:2 — второе');
});
